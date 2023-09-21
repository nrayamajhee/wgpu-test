use crate::{iter_to_array, renderer::Renderer};
use genmesh::{
  generators::{IndexedPolygon, SharedVertex},
  EmitTriangles, Triangulate, Vertex,
};
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::{Float32Array, Object, Uint16Array};
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_buffer_usage, gpu_texture_usage, GpuAddressMode, GpuBindGroup, GpuBindGroupDescriptor,
  GpuBindGroupEntry, GpuBuffer, GpuBufferBinding, GpuBufferDescriptor, GpuFilterMode,
  GpuImageCopyExternalImage, GpuImageCopyTextureTagged, GpuSamplerDescriptor, GpuTextureDescriptor,
  GpuTextureFormat,
};

pub struct Material {
  pub vertex_colors: Vec<[f32; 3]>,
  pub texture_coordinates: Vec<[f32; 2]>,
  pub texture_src: String,
}

pub struct Geometry {
  pub vertices: Vec<[f32; 3]>,
  pub indices: Vec<u16>,
}

impl Geometry {
  pub fn from_genmesh<T, P>(primitive: &T) -> Self
  where
    P: EmitTriangles<Vertex = usize>,
    T: SharedVertex<Vertex> + IndexedPolygon<P>,
  {
    let vertices = primitive
      .shared_vertex_iter()
      .map(|v| v.pos.into())
      .collect();
    let indices: Vec<u16> = primitive
      .indexed_polygon_iter()
      .triangulate()
      .flat_map(|i| [i.x as u16, i.y as u16, i.z as u16])
      .collect();
    Geometry { vertices, indices }
  }
}

pub struct Mesh {
  pub vertext_count: u32,
  pub index_count: u32,
  pub vertex_buffer: GpuBuffer,
  pub vertex_colors: GpuBuffer,
  pub index_buffer: GpuBuffer,
  pub uniform_buffer: GpuBuffer,
  pub uniform_bind_group: GpuBindGroup,
  pub texture_bind_group: GpuBindGroup,
  pub texture_coordinates: GpuBuffer,
}

impl Mesh {
  pub async fn new(
    renderer: &Renderer,
    geometry: &Geometry,
    material: &Material,
  ) -> Result<Self, JsValue> {
    let device = renderer.device();
    let pipeline = renderer.pipeline();
    let size = geometry.vertices.len() * 3 * 4;
    let size = size + 3 & !3;
    let vertex_buffer = {
      let vertex_buffer = device.create_buffer(
        &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
      );
      let write_array = Float32Array::new(&vertex_buffer.get_mapped_range());
      let vertices: Vec<f32> = geometry.vertices.iter().flatten().map(|f| *f).collect();
      write_array.set(&Float32Array::from(&vertices[..]), 0);
      vertex_buffer.unmap();
      vertex_buffer
    };
    let size = geometry.indices.len() * 2;
    let size = size + 3 & !3;
    let index_buffer = {
      let index_buffer = device.create_buffer(
        &GpuBufferDescriptor::new(
          size as f64,
          gpu_buffer_usage::INDEX | gpu_buffer_usage::COPY_DST,
        )
        .mapped_at_creation(true),
      );
      let write_array = Uint16Array::new(&index_buffer.get_mapped_range());
      write_array.set(&Uint16Array::from(&geometry.indices[..]), 0);
      index_buffer.unmap();
      index_buffer
    };
    let size = material.vertex_colors.len() * 3 * 4;
    let size = size + 3 & !3;
    let vertex_colors = {
      let vertex_buffer = device.create_buffer(
        &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
      );
      let write_array = Float32Array::new(&vertex_buffer.get_mapped_range());
      let vertices: Vec<f32> = material
        .vertex_colors
        .iter()
        .flatten()
        .map(|f| *f)
        .collect();
      write_array.set(&Float32Array::from(&vertices[..]), 0);
      vertex_buffer.unmap();
      vertex_buffer
    };
    let uniform_buffer = device.create_buffer(&GpuBufferDescriptor::new(
      16. * 4.,
      gpu_buffer_usage::UNIFORM | gpu_buffer_usage::COPY_DST,
    ));

    let uniform_bind_group = renderer
      .device()
      .create_bind_group(&GpuBindGroupDescriptor::new(
        &iter_to_array(&[JsValue::from(&GpuBindGroupEntry::new(
          0,
          &GpuBufferBinding::new(&uniform_buffer),
        ))]),
        &pipeline.get_bind_group_layout(0),
      ));
    let texture_coordinates = {
      let size = material.texture_coordinates.len() * 2 * 4;
      let size = size + 3 & !3;
      let texure_coordinates = device.create_buffer(
        &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
      );
      let vertices: Vec<f32> = material
        .texture_coordinates
        .iter()
        .flatten()
        .map(|f| *f)
        .collect();
      let write_array = Float32Array::new(&texure_coordinates.get_mapped_range());
      write_array.set(&Float32Array::from(&vertices[..]), 0);
      texure_coordinates.unmap();
      texure_coordinates
    };
    let mut sampler_desc = GpuSamplerDescriptor::new();
    sampler_desc.address_mode_u(GpuAddressMode::Repeat);
    sampler_desc.address_mode_v(GpuAddressMode::Repeat);
    sampler_desc.mag_filter(GpuFilterMode::Linear);
    let sampler = device.create_sampler_with_descriptor(&sampler_desc);
    let image = gloo_utils::document()
      .create_element("img")?
      .dyn_into::<web_sys::HtmlImageElement>()?;
    image.set_attribute("src", &material.texture_src)?;
    let texture = device.create_texture(&GpuTextureDescriptor::new(
      GpuTextureFormat::Rgba8unormSrgb,
      &iter_to_array([image.width() as i32, image.height() as i32]),
      gpu_texture_usage::TEXTURE_BINDING
        | gpu_texture_usage::COPY_DST
        | gpu_texture_usage::RENDER_ATTACHMENT,
    ));
    let bitmap =
      JsFuture::from(window().create_image_bitmap_with_html_image_element(&image)?).await?;
    let mut source = GpuImageCopyExternalImage::new(&Object::new());
    source.flip_y(true);
    source.source(&Object::from(bitmap));
    let rect = &JsValue::from_serde(&Rect {
      width: image.width(),
      height: image.height(),
    })
    .map_err(|e| JsValue::from(format!("{:?}", e)))?;
    device
      .queue()
      .copy_external_image_to_texture_with_u32_sequence(
        &source,
        &GpuImageCopyTextureTagged::new(&texture),
        &rect,
      );
    let texture_bind_group = renderer
      .device()
      .create_bind_group(&GpuBindGroupDescriptor::new(
        &iter_to_array(&[
          JsValue::from(&GpuBindGroupEntry::new(0, &sampler)),
          JsValue::from(&GpuBindGroupEntry::new(1, &texture.create_view())),
        ]),
        &pipeline.get_bind_group_layout(1),
      ));

    Ok(Self {
      vertext_count: geometry.vertices.len() as u32,
      index_count: geometry.indices.len() as u32,
      vertex_buffer,
      index_buffer,
      vertex_colors,
      uniform_buffer,
      uniform_bind_group,
      texture_bind_group,
      texture_coordinates,
    })
  }
}

#[derive(Serialize)]
pub struct Rect {
  width: u32,
  height: u32,
}
