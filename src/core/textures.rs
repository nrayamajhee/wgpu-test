//! Texture upload, caching and mipmap generation.
//!
//! All sampled textures get a full mip chain (generated on the GPU by
//! [`Mipmapper`]) so minified and rough-reflection lookups don't alias.
//! Color textures use `rgba8unorm-srgb` so sampling returns linear values.

use std::collections::HashMap;

use js_sys::{Object, Uint8Array};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_texture_usage, Blob, GpuAddressMode, GpuBindGroupDescriptor, GpuBindGroupEntry, GpuCullMode,
  GpuDevice, GpuFilterMode, GpuImageCopyExternalImage, GpuImageCopyTexture,
  GpuImageCopyTextureTagged, GpuImageDataLayout, GpuLoadOp, GpuRenderPassColorAttachment,
  GpuRenderPassDescriptor, GpuRenderPipeline, GpuSampler, GpuSamplerDescriptor, GpuStoreOp,
  GpuTexture, GpuTextureDescriptor, GpuTextureFormat, GpuTextureViewDescriptor,
  GpuTextureViewDimension, ImageBitmap, Response,
};

use crate::core::{Rect, Renderer, TextureSource};
use crate::utils::iter_to_array;

/// Uploaded textures keyed by source and color space.
///
/// Values keep their [`TextureSource`] alive so `Rc` pointer keys are never
/// reused by a different image.
#[derive(Default)]
pub struct TextureCache {
  /// Key → (source kept alive, texture).
  entries: HashMap<String, (Option<TextureSource>, GpuTexture)>,
}

impl TextureCache {
  /// Cache key for `source` in the given color space.
  fn key(source: &TextureSource, srgb: bool) -> String {
    match source {
      TextureSource::Url(url) => format!("{srgb}:url:{url}"),
      TextureSource::Bytes(bytes) => format!("{srgb}:bytes:{:p}", bytes.as_ptr()),
    }
  }

  /// Creates a 1×1 texture filled with `rgba`, with `layers` array layers
  /// (6 for a cube).
  pub fn solid(device: &GpuDevice, rgba: [u8; 4], srgb: bool, layers: u32) -> GpuTexture {
    let texture = create_texture(device, Rect { width: 1, height: 1 }, layers, format(srgb), 1);
    let mut layout = GpuImageDataLayout::new();
    layout.bytes_per_row(4);
    for layer in 0..layers {
      let mut destination = GpuImageCopyTexture::new(&texture);
      destination.origin(&iter_to_array([0, 0, layer]));
      device.queue().write_texture_with_u8_array_and_u32_sequence(
        &destination,
        &rgba,
        &layout,
        &iter_to_array([1u32, 1]),
      );
    }
    texture
  }
}

/// Texture format for color (`srgb`) or data (linear) textures.
fn format(srgb: bool) -> GpuTextureFormat {
  if srgb {
    GpuTextureFormat::Rgba8unormSrgb
  } else {
    GpuTextureFormat::Rgba8unorm
  }
}

/// Full mip chain length for `rect`.
fn mip_levels(rect: Rect) -> u32 {
  32 - rect.width.max(rect.height).max(1).leading_zeros()
}

/// Creates a sampleable, writable, renderable 2D texture with `layers` array
/// layers and `levels` mip levels.
fn create_texture(
  device: &GpuDevice,
  rect: Rect,
  layers: u32,
  format: GpuTextureFormat,
  levels: u32,
) -> GpuTexture {
  let mut desc = GpuTextureDescriptor::new(
    format,
    &iter_to_array([rect.width, rect.height, layers]),
    gpu_texture_usage::TEXTURE_BINDING
      | gpu_texture_usage::COPY_DST
      | gpu_texture_usage::RENDER_ATTACHMENT,
  );
  desc.mip_level_count(levels);
  device.create_texture(&desc)
}

/// Downsamples mip level `n - 1` into level `n` with a linear-filtered
/// fullscreen triangle (`mipmap.wgsl`), one pass per level and layer.
pub struct Mipmapper {
  /// Pipelines for linear and sRGB targets.
  pipelines: [(GpuTextureFormat, GpuRenderPipeline); 2],
  /// Linear clamp-to-edge sampler.
  sampler: GpuSampler,
}

impl Mipmapper {
  /// Builds the downsampling pipelines.
  pub fn new(device: &GpuDevice) -> Self {
    let pipeline = |format| {
      let pipeline = Renderer::create_pipeline(
        device,
        "Mipmap pipeline",
        include_str!("mipmap.wgsl"),
        &[],
        format,
        GpuCullMode::None,
        None,
      );
      (format, pipeline)
    };
    let mut desc = GpuSamplerDescriptor::new();
    desc
      .address_mode_u(GpuAddressMode::ClampToEdge)
      .address_mode_v(GpuAddressMode::ClampToEdge)
      .mag_filter(GpuFilterMode::Linear)
      .min_filter(GpuFilterMode::Linear);
    Self {
      pipelines: [format(false), format(true)].map(pipeline),
      sampler: device.create_sampler_with_descriptor(&desc),
    }
  }

  /// Fills mip levels `1..` of every layer of `texture` from level 0.
  fn generate(&self, device: &GpuDevice, texture: &GpuTexture) {
    let texture_format = texture.format();
    let Some((_, pipeline)) = self.pipelines.iter().find(|(f, _)| *f == texture_format) else {
      return;
    };
    let view = |level, layer| {
      texture.create_view_with_descriptor(
        GpuTextureViewDescriptor::new()
          .dimension(GpuTextureViewDimension::N2d)
          .base_mip_level(level)
          .mip_level_count(1)
          .base_array_layer(layer)
          .array_layer_count(1),
      )
    };
    let encoder = device.create_command_encoder();
    for layer in 0..texture.depth_or_array_layers() {
      for level in 1..texture.mip_level_count() {
        let bind_group = device.create_bind_group(&GpuBindGroupDescriptor::new(
          &iter_to_array([
            JsValue::from(&GpuBindGroupEntry::new(0, &view(level - 1, layer))),
            JsValue::from(&GpuBindGroupEntry::new(1, &self.sampler)),
          ]),
          &pipeline.get_bind_group_layout(0),
        ));
        let target =
          GpuRenderPassColorAttachment::new(GpuLoadOp::Clear, GpuStoreOp::Store, &view(level, layer));
        let pass = encoder.begin_render_pass(&GpuRenderPassDescriptor::new(&iter_to_array([
          JsValue::from(&target),
        ])));
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, Some(&bind_group));
        pass.draw(3);
        pass.end();
      }
    }
    device.queue().submit(&iter_to_array([encoder.finish()]));
  }
}

impl Renderer {
  /// Uploads (or reuses from cache) the texture for `source`, with mipmaps.
  /// `srgb` for color data (base color), linear for everything else.
  ///
  /// # Errors
  /// If the image fails to fetch or decode.
  pub async fn texture(&self, source: &TextureSource, srgb: bool) -> Result<GpuTexture, JsValue> {
    let key = TextureCache::key(source, srgb);
    if let Some((_, texture)) = self.texture_cache().borrow().entries.get(&key) {
      return Ok(texture.clone());
    }
    let (bitmap, rect) = match source {
      TextureSource::Url(url) => Self::create_bitmap(url).await?,
      TextureSource::Bytes(bytes) => Self::create_bitmap_from_bytes(bytes).await?,
    };
    let texture = self.texture_from_bitmaps(&[bitmap], rect, srgb);
    self
      .texture_cache()
      .borrow_mut()
      .entries
      .insert(key, (Some(source.clone()), texture.clone()));
    Ok(texture)
  }

  /// Cached 1×1 texture of `rgba`, used as a default for missing maps.
  pub fn solid_texture(&self, rgba: [u8; 4], srgb: bool) -> GpuTexture {
    let key = format!("{srgb}:solid:{rgba:?}");
    let mut cache = self.texture_cache().borrow_mut();
    cache
      .entries
      .entry(key)
      .or_insert_with(|| (None, TextureCache::solid(self.device(), rgba, srgb, 1)))
      .1
      .clone()
  }

  /// Fetches six face images (+x −x +y −y +z −z) into one mipmapped, sRGB,
  /// 6-layer texture, to be viewed as a cube.
  ///
  /// # Errors
  /// If any face fails to fetch or decode.
  pub async fn cubemap_texture(&self, faces: &[String]) -> Result<GpuTexture, JsValue> {
    let mut bitmaps = Vec::with_capacity(faces.len());
    let mut rect = Rect { width: 1, height: 1 };
    for (i, face) in faces.iter().enumerate() {
      let (bitmap, r) = Self::create_bitmap(face).await?;
      if i == 0 {
        rect = r;
      }
      bitmaps.push(bitmap);
    }
    Ok(self.texture_from_bitmaps(&bitmaps, rect, true))
  }

  /// Copies each bitmap into its own array layer at mip 0, then generates mips.
  fn texture_from_bitmaps(&self, bitmaps: &[ImageBitmap], rect: Rect, srgb: bool) -> GpuTexture {
    let texture = create_texture(
      self.device(),
      rect,
      bitmaps.len() as u32,
      format(srgb),
      mip_levels(rect),
    );
    for (layer, bitmap) in bitmaps.iter().enumerate() {
      let mut source = GpuImageCopyExternalImage::new(&Object::new());
      source.flip_y(false);
      source.source(&Object::from(bitmap.clone()));
      let mut destination = GpuImageCopyTextureTagged::new(&texture);
      destination.origin(&iter_to_array([0, 0, layer as u32]));
      self
        .device()
        .queue()
        .copy_external_image_to_texture_with_u32_sequence(
          &source,
          &destination,
          &iter_to_array([rect.width, rect.height]),
        );
    }
    self.mipmapper().generate(self.device(), &texture);
    texture
  }

  /// Fetches and decodes the image at URL `src`.
  ///
  /// # Errors
  /// If the fetch or decode fails.
  pub async fn create_bitmap(src: &str) -> Result<(ImageBitmap, Rect), JsValue> {
    let res = JsFuture::from(gloo_utils::window().fetch_with_str(src))
      .await?
      .dyn_into::<Response>()?;
    let blob = JsFuture::from(res.blob()?).await?.dyn_into::<Blob>()?;
    Self::bitmap_from_blob(blob).await
  }

  /// Decodes encoded image `bytes` (PNG, JPEG, …).
  ///
  /// # Errors
  /// If the bytes aren't a decodable image.
  pub async fn create_bitmap_from_bytes(bytes: &[u8]) -> Result<(ImageBitmap, Rect), JsValue> {
    let parts = js_sys::Array::new();
    parts.push(&Uint8Array::from(bytes));
    let blob = Blob::new_with_u8_array_sequence(&parts)?;
    Self::bitmap_from_blob(blob).await
  }

  /// Decodes `blob` into an [`ImageBitmap`] and its size.
  async fn bitmap_from_blob(blob: Blob) -> Result<(ImageBitmap, Rect), JsValue> {
    let bitmap =
      JsFuture::from(gloo_utils::window().create_image_bitmap_with_blob(&blob)?).await?;
    let image = bitmap.dyn_into::<ImageBitmap>()?;
    let (width, height) = (image.width(), image.height());
    Ok((image, Rect { width, height }))
  }
}
