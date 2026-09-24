//! WebGPU setup, pipelines and per-frame drawing.
//!
//! Pipelines (see `docs/shaders.md`):
//! - **PBR** (`pbr.wgsl`): every regular mesh; lit by scene [lights](crate::lights)
//!   and reflects the environment cube map.
//! - **Cube map** (`shader_cube.wgsl`): skybox at infinite depth.
//! - **Mipmap** (`mipmap.wgsl`): internal, downsamples texture mip levels.
//!
//! The canvas is rendered through an `-srgb` view, so shaders output linear
//! color and the GPU encodes it.

use std::cell::RefCell;

use crate::core::textures::{Mipmapper, TextureCache};
use crate::core::{MaterialType, Scene, Viewport};
use crate::lights::{Light, MAX_POINT_LIGHTS, MAX_SUNS};
use crate::utils::iter_to_array;
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::{ArrayBuffer, Float32Array, Uint8Array};
use nalgebra::Similarity3;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_buffer_usage, gpu_texture_usage, GpuAdapter, GpuAddressMode, GpuBindGroup,
  GpuBindGroupDescriptor, GpuBindGroupEntry, GpuBindGroupLayout, GpuBuffer, GpuBufferBinding,
  GpuBufferDescriptor, GpuCanvasAlphaMode, GpuCanvasConfiguration, GpuCanvasContext,
  GpuColorTargetState, GpuCompareFunction, GpuCullMode, GpuDepthStencilState, GpuDevice,
  GpuFilterMode, GpuFragmentState, GpuFrontFace, GpuIndexFormat, GpuLoadOp, GpuMipmapFilterMode,
  GpuPrimitiveState, GpuPrimitiveTopology, GpuRenderPassColorAttachment,
  GpuRenderPassDepthStencilAttachment, GpuRenderPassDescriptor, GpuRenderPipeline,
  GpuRenderPipelineDescriptor, GpuSampler, GpuSamplerDescriptor, GpuShaderModuleDescriptor,
  GpuStoreOp, GpuTexture, GpuTextureDescriptor, GpuTextureFormat, GpuTextureViewDescriptor,
  GpuTextureViewDimension, GpuVertexAttribute, GpuVertexBufferLayout, GpuVertexFormat,
  GpuVertexState, HtmlCanvasElement, Response,
};

/// Size of the per-frame uniform block (`Frame` in `pbr.wgsl`): view-projection,
/// camera position, ambient, counts, [`MAX_SUNS`] suns and
/// [`MAX_POINT_LIGHTS`] point lights (two `vec4`s each).
const FRAME_UNIFORM_SIZE: usize = 4 * (16 + 4 * 3 + 8 * (MAX_SUNS + MAX_POINT_LIGHTS));

/// Depth buffer format shared by all pipelines.
const DEPTH_FORMAT: GpuTextureFormat = GpuTextureFormat::Depth24plusStencil8;

/// Vertex buffer layout: `(stride in bytes, [(format, shader location)])`.
type BufferLayout<'a> = (f64, &'a [(GpuVertexFormat, u32)]);

/// Owns the canvas, GPU device, pipelines, render targets and shared GPU state.
pub struct Renderer {
  /// Full-window canvas being drawn to.
  canvas: HtmlCanvasElement,
  /// WebGPU context of `canvas`; provides the swap-chain texture.
  context: GpuCanvasContext,
  /// Logical GPU device.
  device: GpuDevice,
  /// `-srgb` view format of the canvas, used as the render target format.
  surface_format: GpuTextureFormat,
  /// PBR mesh pipeline.
  pipeline_pbr: GpuRenderPipeline,
  /// Skybox (cube map) pipeline.
  pipeline_cubemap: GpuRenderPipeline,
  /// Depth/stencil target sized to the canvas. Kept alive for `depth_attachment`.
  depth_texture: GpuTexture,
  /// Color attachment; its view is swapped each frame.
  color_attachment: GpuRenderPassColorAttachment,
  /// Depth attachment, cleared each frame.
  depth_attachment: GpuRenderPassDepthStencilAttachment,
  /// Reused render pass descriptor.
  render_pass_descriptor: GpuRenderPassDescriptor,
  /// Shared trilinear, anisotropic, repeating sampler.
  sampler: GpuSampler,
  /// PBR group 0 uniforms: camera and lights.
  frame_buffer: GpuBuffer,
  /// PBR group 0: `frame_buffer`, sampler, environment cube.
  frame_bind_group: GpuBindGroup,
  /// Cube texture reflected by PBR surfaces: the scene's skybox once
  /// loaded, a black 1×1 cube until then.
  environment: GpuTexture,
  /// Uploaded textures by source, so shared images upload once.
  textures: RefCell<TextureCache>,
  /// Mip level generation.
  mipmapper: Mipmapper,
}

impl Renderer {
  /// Creates a `Depth24plusStencil8` texture and a clearing attachment for it.
  fn create_depth_texture(
    device: &GpuDevice,
    width: u32,
    height: u32,
  ) -> (GpuTexture, GpuRenderPassDepthStencilAttachment) {
    let depth_texture = device.create_texture(&GpuTextureDescriptor::new(
      DEPTH_FORMAT,
      &iter_to_array([width, height]),
      gpu_texture_usage::RENDER_ATTACHMENT,
    ));
    let mut depth_attachment =
      GpuRenderPassDepthStencilAttachment::new(&depth_texture.create_view());
    depth_attachment
      .depth_clear_value(1.)
      .depth_load_op(GpuLoadOp::Clear)
      .depth_store_op(GpuStoreOp::Store)
      .stencil_clear_value(0)
      .stencil_load_op(GpuLoadOp::Clear)
      .stencil_store_op(GpuStoreOp::Store);
    (depth_texture, depth_attachment)
  }

  /// Builds a triangle-list render pipeline with an `"auto"` layout,
  /// entry points `vs_main` / `fs_main`.
  ///
  /// `depth` is `(compare, write)`; `None` disables depth testing.
  pub fn create_pipeline(
    device: &GpuDevice,
    label: &str,
    shader: &str,
    buffers: &[BufferLayout],
    target: GpuTextureFormat,
    cull: GpuCullMode,
    depth: Option<(GpuCompareFunction, bool)>,
  ) -> GpuRenderPipeline {
    let module = device.create_shader_module(&GpuShaderModuleDescriptor::new(shader));
    let mut vertex = GpuVertexState::new(&module);
    vertex.entry_point("vs_main");
    vertex.buffers(&iter_to_array(buffers.iter().map(|(stride, attributes)| {
      GpuVertexBufferLayout::new(
        *stride,
        &iter_to_array(
          attributes
            .iter()
            .map(|&(format, location)| GpuVertexAttribute::new(format, 0., location)),
        ),
      )
    })));
    let mut fragment = GpuFragmentState::new(
      &module,
      &iter_to_array([GpuColorTargetState::new(target)]),
    );
    fragment.entry_point("fs_main");
    let mut descriptor = GpuRenderPipelineDescriptor::new(&"auto".into(), &vertex);
    descriptor
      .label(label)
      .fragment(&fragment)
      .primitive(
        GpuPrimitiveState::new()
          .front_face(GpuFrontFace::Ccw)
          .cull_mode(cull)
          .topology(GpuPrimitiveTopology::TriangleList),
      );
    if let Some((compare, write)) = depth {
      descriptor.depth_stencil(
        GpuDepthStencilState::new(DEPTH_FORMAT)
          .depth_compare(compare)
          .depth_write_enabled(write),
      );
    }
    device.create_render_pipeline(&descriptor)
  }

  /// Creates a window-sized canvas (not yet attached to the DOM), requests a
  /// GPU device and builds all pipelines.
  ///
  /// # Errors
  /// If WebGPU is unsupported or adapter/device/context acquisition fails.
  pub async fn new() -> Result<Self, JsValue> {
    let canvas = window()
      .document()
      .unwrap()
      .create_element("canvas")?
      .dyn_into::<HtmlCanvasElement>()?;
    let gpu = window().navigator().gpu();
    let adapter = JsFuture::from(gpu.request_adapter())
      .await?
      .dyn_into::<GpuAdapter>()?;
    let device = JsFuture::from(adapter.request_device())
      .await?
      .dyn_into::<GpuDevice>()?;
    let context = canvas
      .get_context("webgpu")?
      .unwrap()
      .dyn_into::<GpuCanvasContext>()?;
    let (width, height) = get_window_dimension();
    canvas.set_width(width);
    canvas.set_height(height);

    let canvas_format = gpu.get_preferred_canvas_format();
    let surface_format = match canvas_format {
      GpuTextureFormat::Bgra8unorm => GpuTextureFormat::Bgra8unormSrgb,
      GpuTextureFormat::Rgba8unorm => GpuTextureFormat::Rgba8unormSrgb,
      other => other,
    };
    let mut ctx_config = GpuCanvasConfiguration::new(&device, canvas_format);
    ctx_config
      .alpha_mode(GpuCanvasAlphaMode::Premultiplied)
      .view_formats(&iter_to_array([JsValue::from(surface_format)]));
    context.configure(&ctx_config);

    let mut color_attachment = GpuRenderPassColorAttachment::new(
      GpuLoadOp::Clear,
      GpuStoreOp::Store,
      &context.get_current_texture().create_view(),
    );
    color_attachment.clear_value(&JsValue::from_serde(&Color::rgb(0., 0., 0.)).unwrap());
    let (depth_texture, depth_attachment) = Self::create_depth_texture(&device, width, height);
    let mut render_pass_descriptor =
      GpuRenderPassDescriptor::new(&iter_to_array([JsValue::from(&color_attachment)]));
    render_pass_descriptor.depth_stencil_attachment(&depth_attachment);

    use GpuVertexFormat::{Float32x2, Float32x3, Float32x4};
    let pipeline_pbr = Self::create_pipeline(
      &device,
      "PBR pipeline",
      include_str!("pbr.wgsl"),
      &[
        (12., &[(Float32x3, 0)]), // position
        (12., &[(Float32x3, 1)]), // normal
        (16., &[(Float32x4, 2)]), // tangent
        (8., &[(Float32x2, 3)]),  // uv
        (12., &[(Float32x3, 4)]), // color
      ],
      surface_format,
      GpuCullMode::Back,
      Some((GpuCompareFunction::Less, true)),
    );
    // Skybox sits at depth 1 (the far plane): drawn wherever nothing else
    // is, in any order, without writing depth.
    let pipeline_cubemap = Self::create_pipeline(
      &device,
      "Cube map pipeline",
      include_str!("shader_cube.wgsl"),
      &[(12., &[(Float32x3, 0)])],
      surface_format,
      GpuCullMode::Front,
      Some((GpuCompareFunction::LessEqual, false)),
    );

    let mut sampler_desc = GpuSamplerDescriptor::new();
    sampler_desc
      .address_mode_u(GpuAddressMode::Repeat)
      .address_mode_v(GpuAddressMode::Repeat)
      .mag_filter(GpuFilterMode::Linear)
      .min_filter(GpuFilterMode::Linear)
      .mipmap_filter(GpuMipmapFilterMode::Linear)
      .max_anisotropy(8);
    let sampler = device.create_sampler_with_descriptor(&sampler_desc);

    let frame_buffer = device.create_buffer(&GpuBufferDescriptor::new(
      FRAME_UNIFORM_SIZE as f64,
      gpu_buffer_usage::UNIFORM | gpu_buffer_usage::COPY_DST,
    ));
    let mipmapper = Mipmapper::new(&device);
    let environment = TextureCache::solid(&device, [0, 0, 0, 255], true, 6);
    let frame_bind_group =
      Self::frame_bind_group(&device, &pipeline_pbr, &frame_buffer, &sampler, &environment);

    Ok(Self {
      canvas,
      context,
      device,
      surface_format,
      pipeline_pbr,
      pipeline_cubemap,
      depth_texture,
      color_attachment,
      depth_attachment,
      render_pass_descriptor,
      sampler,
      frame_buffer,
      frame_bind_group,
      environment,
      textures: RefCell::new(TextureCache::default()),
      mipmapper,
    })
  }

  /// Builds PBR bind group 0 for `environment`.
  fn frame_bind_group(
    device: &GpuDevice,
    pipeline: &GpuRenderPipeline,
    frame_buffer: &GpuBuffer,
    sampler: &GpuSampler,
    environment: &GpuTexture,
  ) -> GpuBindGroup {
    let cube_view = environment.create_view_with_descriptor(
      GpuTextureViewDescriptor::new().dimension(GpuTextureViewDimension::Cube),
    );
    device.create_bind_group(&GpuBindGroupDescriptor::new(
      &iter_to_array([
        JsValue::from(&GpuBindGroupEntry::new(0, &GpuBufferBinding::new(frame_buffer))),
        JsValue::from(&GpuBindGroupEntry::new(1, sampler)),
        JsValue::from(&GpuBindGroupEntry::new(2, &cube_view)),
      ]),
      &pipeline.get_bind_group_layout(0),
    ))
  }

  /// Makes PBR surfaces reflect `environment` (a cube texture).
  fn set_environment(&mut self, environment: GpuTexture) {
    self.frame_bind_group = Self::frame_bind_group(
      &self.device,
      &self.pipeline_pbr,
      &self.frame_buffer,
      &self.sampler,
      &environment,
    );
    self.environment = environment;
  }

  /// Shared trilinear, anisotropic, repeating sampler.
  pub fn texture_sampler(&self) -> &GpuSampler {
    &self.sampler
  }
  /// The canvas being rendered to.
  pub fn canvas(&self) -> &HtmlCanvasElement {
    &self.canvas
  }
  /// The GPU device, for creating resources.
  pub fn device(&self) -> &GpuDevice {
    &self.device
  }
  /// PBR pipeline (for bind group layouts).
  pub fn pipeline_pbr(&self) -> &GpuRenderPipeline {
    &self.pipeline_pbr
  }
  /// Cube map pipeline (for bind group layouts).
  pub fn pipeline_cubemap(&self) -> &GpuRenderPipeline {
    &self.pipeline_cubemap
  }
  /// Uploaded-texture cache.
  pub fn texture_cache(&self) -> &RefCell<TextureCache> {
    &self.textures
  }
  /// Mip level generator.
  pub fn mipmapper(&self) -> &Mipmapper {
    &self.mipmapper
  }

  /// Draws `scene` from `viewport` in one render pass.
  ///
  /// Adopts the scene's cube map (if any) as the PBR environment, uploads
  /// camera and light uniforms, then each mesh's uniforms, then draws.
  pub fn render(&mut self, scene: &Scene, viewport: &Viewport) {
    let meshes = scene.meshes();
    if let Some(environment) = meshes.iter().find_map(|(m, _)| m.environment.as_ref()) {
      if *environment != self.environment {
        self.set_environment(environment.clone());
      }
    }
    self.write_frame_uniforms(&scene.lights(), viewport);

    let queue = self.device.queue();
    self.color_attachment.view(
      &self
        .context
        .get_current_texture()
        .create_view_with_descriptor(GpuTextureViewDescriptor::new().format(self.surface_format)),
    );
    self
      .render_pass_descriptor
      .color_attachments(&iter_to_array([JsValue::from(&self.color_attachment)]));
    self
      .render_pass_descriptor
      .depth_stencil_attachment(&self.depth_attachment);
    let command_encoder = self.device.create_command_encoder();
    let pass = command_encoder.begin_render_pass(&self.render_pass_descriptor);
    let (width, height) = (self.canvas.width(), self.canvas.height());
    pass.set_viewport(0., 0., width as f32, height as f32, 0., 1.);
    pass.set_scissor_rect(0, 0, width, height);

    for (mesh, transform) in meshes.iter() {
      let uniforms: Vec<f32> = match mesh.material_type {
        MaterialType::CubeMap => {
          pass.set_pipeline(&self.pipeline_cubemap);
          pass.set_bind_group(0, Some(&mesh.bind_groups[0]));
          pass.set_bind_group(1, Some(&mesh.bind_groups[1]));
          (viewport.view_cube() * transform.to_homogeneous())
            .as_slice()
            .to_vec()
        }
        MaterialType::Pbr => {
          pass.set_pipeline(&self.pipeline_pbr);
          pass.set_bind_group(0, Some(&self.frame_bind_group));
          pass.set_bind_group(1, Some(&mesh.bind_groups[0]));
          let (c, e) = (mesh.color, mesh.emissive);
          let mut data = transform.to_homogeneous().as_slice().to_vec();
          data.extend([c.r, c.g, c.b, c.a]);
          data.extend([mesh.metallic, mesh.roughness, mesh.normal_scale, mesh.occlusion_strength]);
          data.extend([e.r, e.g, e.b, 0.]);
          data
        }
      };
      queue.write_buffer_with_u32_and_buffer_source(
        &mesh.uniform_buffer,
        0,
        &Float32Array::from(&uniforms[..]),
      );
      for (slot, buffer) in mesh.vertex_buffers.iter().enumerate() {
        pass.set_vertex_buffer(slot as u32, Some(buffer));
      }
      pass.set_index_buffer(&mesh.index_buffer, GpuIndexFormat::Uint32);
      pass.draw_indexed(mesh.index_count);
    }
    pass.end();
    queue.submit(&iter_to_array([command_encoder.finish()]));
  }

  /// Packs camera, lights and environment info into `frame_buffer`
  /// (layout of `Frame` in `pbr.wgsl`). Lights beyond the shader's limits
  /// are dropped; ambients are summed.
  fn write_frame_uniforms(&self, lights: &[(&Light, Similarity3<f32>)], viewport: &Viewport) {
    let mut ambient = [0f32; 3];
    let mut suns = Vec::with_capacity(MAX_SUNS * 8);
    let mut points = Vec::with_capacity(MAX_POINT_LIGHTS * 8);
    for (light, transform) in lights {
      match light {
        Light::Ambient(a) => {
          ambient[0] += a.color.r * a.intensity;
          ambient[1] += a.color.g * a.intensity;
          ambient[2] += a.color.b * a.intensity;
        }
        Light::Sun(s) if suns.len() < MAX_SUNS * 8 => {
          let d = transform.isometry.rotation * s.direction.into_inner();
          let c = s.color;
          suns.extend([d.x, d.y, d.z, 0.]);
          suns.extend([c.r * s.intensity, c.g * s.intensity, c.b * s.intensity, 0.]);
        }
        Light::Point(p) if points.len() < MAX_POINT_LIGHTS * 8 => {
          let pos = transform.isometry.translation.vector;
          let c = p.color;
          points.extend([pos.x, pos.y, pos.z, p.range]);
          points.extend([c.r * p.intensity, c.g * p.intensity, c.b * p.intensity, 0.]);
        }
        _ => {}
      }
    }
    let eye = viewport.eye_position();
    let max_mip = self.environment.mip_level_count().saturating_sub(1) as f32;

    let mut data = Vec::with_capacity(FRAME_UNIFORM_SIZE / 4);
    data.extend_from_slice(viewport.view_proj().as_slice());
    data.extend([eye.x, eye.y, eye.z, 1.]);
    data.extend([ambient[0], ambient[1], ambient[2], 0.]);
    data.extend([(suns.len() / 8) as f32, (points.len() / 8) as f32, max_mip, 0.]);
    suns.resize(MAX_SUNS * 8, 0.);
    points.resize(MAX_POINT_LIGHTS * 8, 0.);
    data.extend(suns);
    data.extend(points);
    self.device.queue().write_buffer_with_u32_and_buffer_source(
      &self.frame_buffer,
      0,
      &Float32Array::from(&data[..]),
    );
  }

  /// Resizes the canvas and depth texture to the current window size.
  pub fn resize(&mut self) {
    let (width, height) = get_window_dimension();
    self.canvas.set_width(width);
    self.canvas.set_height(height);
    let (depth_texture, depth_attachment) = Self::create_depth_texture(&self.device, width, height);
    self.depth_texture = depth_texture;
    self.depth_attachment = depth_attachment;
  }

  /// Creates a bind group from `entries` for `layout`.
  pub fn create_bind_group(
    &self,
    layout: &GpuBindGroupLayout,
    entries: &[GpuBindGroupEntry],
  ) -> GpuBindGroup {
    self.device.create_bind_group(&GpuBindGroupDescriptor::new(
      &iter_to_array(entries.iter().map(JsValue::from)),
      layout,
    ))
  }

  /// Creates a zeroed uniform buffer of `size` bytes, writable via the queue.
  pub fn create_uniform_buffer(&self, size: f64) -> GpuBuffer {
    self.device.create_buffer(&GpuBufferDescriptor::new(
      size,
      gpu_buffer_usage::UNIFORM | gpu_buffer_usage::COPY_DST,
    ))
  }

  /// Creates a vertex buffer initialised with `data` (size padded to 4 bytes).
  pub fn create_buffer(&self, data: &[f32]) -> GpuBuffer {
    let byte_len = data.len() * 4;
    let size = (byte_len + 3) & !3;
    let buffer = self.device.create_buffer(
      GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
    );
    let write_array = Float32Array::new(&buffer.get_mapped_range());
    write_array.set(&Float32Array::from(data), 0);
    buffer.unmap();
    buffer
  }

  /// Creates a `u32` index buffer initialised with `data`.
  pub fn create_index_buffer(&self, data: &[u32]) -> GpuBuffer {
    let size = data.len() * 4;
    let size = (size + 3) & !3;
    let buffer = self.device.create_buffer(
      GpuBufferDescriptor::new(
        size as f64,
        gpu_buffer_usage::INDEX | gpu_buffer_usage::COPY_DST,
      )
      .mapped_at_creation(true),
    );
    let write_array = js_sys::Uint32Array::new(&buffer.get_mapped_range());
    write_array.set(&js_sys::Uint32Array::from(data), 0);
    buffer.unmap();
    buffer
  }

  /// Fetches URL `src` as raw bytes (e.g. a `.glb`).
  ///
  /// # Errors
  /// If the request fails.
  pub async fn fetch_bytes(src: &str) -> Result<Vec<u8>, JsValue> {
    let res = JsFuture::from(window().fetch_with_str(src))
      .await?
      .dyn_into::<Response>()?;
    let buf = JsFuture::from(res.array_buffer()?)
      .await?
      .dyn_into::<ArrayBuffer>()?;
    Ok(Uint8Array::new(&buf).to_vec())
  }
}

/// Browser window inner `(width, height)` in CSS pixels.
pub fn get_window_dimension() -> (u32, u32) {
  let window = window();
  (
    window
      .inner_width()
      .expect("Window has no width")
      .as_f64()
      .expect("Width isn't f64") as u32,
    window
      .inner_height()
      .expect("Window has no height")
      .as_f64()
      .expect("Height isn't f64") as u32,
  )
}

/// Linear RGBA color, components in `0.0..=1.0`.
/// Serializes to a WebGPU `GPUColor` dictionary.
#[derive(Serialize, Clone, Copy, Debug)]
pub struct Color {
  /// Red.
  pub r: f32,
  /// Green.
  pub g: f32,
  /// Blue.
  pub b: f32,
  /// Alpha.
  pub a: f32,
}

impl Color {
  /// Opaque color.
  pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
    Self { r, g, b, a: 1. }
  }
}

/// Pixel dimensions of an image or texture.
#[derive(Serialize, Clone, Copy)]
pub struct Rect {
  /// Width in pixels.
  pub width: u32,
  /// Height in pixels.
  pub height: u32,
}
