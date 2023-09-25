use crate::Color;
use crate::{iter_to_array, renderer::Renderer};
use genmesh::{
  generators::{IndexedPolygon, SharedVertex},
  EmitTriangles, Triangulate, Vertex,
};
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::{Float32Array, Int8Array, Object, Uint16Array, Uint8Array};
use serde::Serialize;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_buffer_usage, gpu_texture_usage, Blob, GpuBindGroup, GpuBindGroupDescriptor,
  GpuBindGroupEntry, GpuBuffer, GpuBufferBinding, GpuBufferDescriptor, GpuDevice,
  GpuImageCopyExternalImage, GpuImageCopyTextureTagged, GpuTexture, GpuTextureDescriptor,
  GpuTextureFormat, ImageBitmap, Response,
};

#[derive(PartialEq, Clone, Copy)]
pub enum MaterialType {
  Color = 0,
  VertexColor = 1,
  Textured = 2,
}

pub struct Material {
  pub material_type: MaterialType,
  pub vertex_colors: Vec<[f32; 3]>,
  pub texture_coordinates: Vec<[f32; 2]>,
  pub texture_src: String,
  pub color: Color,
}

impl Material {
  pub fn new(color: Color) -> Self {
    Self {
      material_type: MaterialType::Color,
      vertex_colors: vec![],
      texture_coordinates: vec![],
      texture_src: "".to_string(),
      color,
    }
  }
  pub fn vertex_coolor(colors: Vec<[f32; 3]>) -> Self {
    Self {
      material_type: MaterialType::VertexColor,
      vertex_colors: colors,
      texture_coordinates: vec![],
      texture_src: "".to_string(),
      color: Color {
        r: 1.,
        g: 1.,
        b: 1.,
        a: 1.,
      },
    }
  }
  pub fn textured(src: &str, coordinates: Vec<[f32; 2]>) -> Self {
    Self {
      material_type: MaterialType::Textured,
      vertex_colors: vec![],
      texture_coordinates: coordinates,
      texture_src: src.to_string(),
      color: Color {
        r: 1.,
        g: 1.,
        b: 1.,
        a: 1.,
      },
    }
  }
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
  pub index_buffer: GpuBuffer,
  pub uniform_buffer: GpuBuffer,
  pub uniform_bind_group: GpuBindGroup,

  pub material_buffer: GpuBuffer,
  pub color: Color,
  pub material_type: MaterialType,
  pub vertex_colors: GpuBuffer,
  pub texture_coordinates: GpuBuffer,

  pub texture_bind_group: Option<GpuBindGroup>,
  pub material_bind_group: GpuBindGroup,
}

impl Mesh {
  pub async fn new(
    renderer: &Renderer,
    geometry: &Geometry,
    material: &Material,
  ) -> Result<Self, JsValue> {
    let device = renderer.device();
    let pipeline = renderer.pipeline();
    let vertex_buffer = {
      let vertices: Vec<f32> = geometry.vertices.iter().flatten().map(|f| *f).collect();
      create_buffer(device, &vertices)
    };
    let index_buffer = create_index_buffer(device, &geometry.indices);
    let vertex_colors = if material.material_type == MaterialType::VertexColor {
      let vertices: Vec<f32> = material
        .vertex_colors
        .iter()
        .flatten()
        .map(|f| *f)
        .collect();
      create_buffer(device, &vertices)
    } else {
      create_buffer(device, &[])
    };
    let texture_coordinates = if material.material_type == MaterialType::Textured {
      let vertices: Vec<f32> = material
        .texture_coordinates
        .iter()
        .map(|f| *f)
        .flatten()
        .collect();
      create_buffer(device, &vertices[..])
    } else {
      create_buffer(device, &[])
    };

    let texture_bind_group = if material.material_type == MaterialType::Textured {
      let (texture, source, rect) = create_image(device, &material.texture_src).await?;
      device
        .queue()
        .copy_external_image_to_texture_with_u32_sequence(
          &source,
          &GpuImageCopyTextureTagged::new(&texture),
          &JsValue::from_serde(&rect).unwrap(),
        );
      let texture_binding_group =
        renderer
          .device()
          .create_bind_group(&GpuBindGroupDescriptor::new(
            &iter_to_array(&[
              JsValue::from(&GpuBindGroupEntry::new(0, &renderer.texture_sampler())),
              JsValue::from(&GpuBindGroupEntry::new(1, &texture.create_view())),
            ]),
            &pipeline.get_bind_group_layout(1),
          ));
      Some(texture_binding_group)
    } else {
      None
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

    // let size = 4 * 4 + 4;
    let size = 32;
    let material_buffer = device.create_buffer(&GpuBufferDescriptor::new(
      size as f64,
      gpu_buffer_usage::UNIFORM | gpu_buffer_usage::COPY_DST,
    ));

    let material_bind_group = renderer
      .device()
      .create_bind_group(&GpuBindGroupDescriptor::new(
        &iter_to_array(&[JsValue::from(&GpuBindGroupEntry::new(
          0,
          &GpuBufferBinding::new(&material_buffer),
        ))]),
        &pipeline.get_bind_group_layout(2),
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
      material_type: material.material_type,
      material_buffer,
      material_bind_group,
      color: material.color,
    })
  }
}

fn create_buffer(device: &GpuDevice, data: &[f32]) -> GpuBuffer {
  let byte_len = data.len() * 4;
  let size = byte_len + 3 & !3;
  let buffer = device.create_buffer(
    &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
  );
  let write_array = Float32Array::new(&buffer.get_mapped_range());
  write_array.set(&Float32Array::from(&data[..]), 0);
  buffer.unmap();
  buffer
}

fn create_index_buffer(device: &GpuDevice, data: &[u16]) -> GpuBuffer {
  let size = data.len() * 2;
  let size = size + 3 & !3;
  let buffer = device.create_buffer(
    &GpuBufferDescriptor::new(
      size as f64,
      gpu_buffer_usage::INDEX | gpu_buffer_usage::COPY_DST,
    )
    .mapped_at_creation(true),
  );
  let write_array = Uint16Array::new(&buffer.get_mapped_range());
  write_array.set(&Uint16Array::from(&data[..]), 0);
  buffer.unmap();
  buffer
}
async fn create_image(
  device: &GpuDevice,
  src: &str,
) -> Result<(GpuTexture, GpuImageCopyExternalImage, Rect), JsValue> {
  let res = JsFuture::from(window().fetch_with_str(src))
    .await?
    .dyn_into::<Response>()?;
  let blob = JsFuture::from(res.blob()?).await?.dyn_into::<Blob>()?;
  let bitmap = JsFuture::from(window().create_image_bitmap_with_blob(&blob)?).await?;
  let image = bitmap.dyn_into::<ImageBitmap>()?;
  let (width, height) = (image.width(), image.height());
  let mut source = GpuImageCopyExternalImage::new(&Object::new());
  source.flip_y(false);
  source.source(&Object::from(image));
  Ok((
    device.create_texture(&GpuTextureDescriptor::new(
      GpuTextureFormat::Rgba8unormSrgb,
      &iter_to_array([width as i32, height as i32]),
      gpu_texture_usage::TEXTURE_BINDING
        | gpu_texture_usage::COPY_DST
        | gpu_texture_usage::RENDER_ATTACHMENT,
    )),
    source,
    Rect { width, height },
  ))
}

#[derive(Serialize)]
pub struct Rect {
  width: u32,
  height: u32,
}
