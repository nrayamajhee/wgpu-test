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
        g: 0.,
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
  pub material_type: MaterialType,
  pub color: Color,

  pub vertex_buffer: GpuBuffer,
  pub index_buffer: GpuBuffer,
  pub vertex_colors: GpuBuffer,

  pub uniform_buffer: GpuBuffer,
  pub uniform_bind_group: GpuBindGroup,

  pub material_buffer: GpuBuffer,
  pub material_bind_group: GpuBindGroup,

  pub texture_coordinates: GpuBuffer,
  pub texture_bind_group: Option<GpuBindGroup>,
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
      renderer.create_buffer(&vertices)
    };
    let index_buffer = renderer.create_index_buffer(&geometry.indices);
    let vertex_colors = if material.material_type == MaterialType::VertexColor {
      let vertices: Vec<f32> = material
        .vertex_colors
        .iter()
        .flatten()
        .map(|f| *f)
        .collect();
      renderer.create_buffer(&vertices)
    } else {
      renderer.create_buffer(&[])
    };
    let texture_coordinates = if material.material_type == MaterialType::Textured {
      let vertices: Vec<f32> = material
        .texture_coordinates
        .iter()
        .map(|f| *f)
        .flatten()
        .collect();
      renderer.create_buffer(&vertices[..])
    } else {
      renderer.create_buffer(&[])
    };

    let texture_bind_group = if material.material_type == MaterialType::Textured {
      let (texture, source, rect) = renderer.create_image(&material.texture_src).await?;
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
      material_type: material.material_type,
      color: material.color,
      vertex_buffer,
      index_buffer,
      vertex_colors,
      uniform_buffer,
      uniform_bind_group,
      texture_coordinates,
      texture_bind_group,
      material_buffer,
      material_bind_group,
    })
  }
}
