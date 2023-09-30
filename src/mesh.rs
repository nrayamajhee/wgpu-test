use crate::renderer::Rect;
use crate::Color;
use crate::{iter_to_array, renderer::Renderer};
use genmesh::{
  generators::{IndexedPolygon, SharedVertex},
  EmitTriangles, Triangulate, Vertex,
};
use gloo_utils::format::JsValueSerdeExt;
use js_sys::Object;
use wasm_bindgen::JsValue;
use web_sys::{
  gpu_buffer_usage, GpuBindGroup, GpuBindGroupDescriptor, GpuBindGroupEntry, GpuBuffer,
  GpuBufferBinding, GpuBufferDescriptor, GpuImageCopyExternalImage, GpuImageCopyTextureTagged,
  GpuTextureViewDescriptor, GpuTextureViewDimension,
};

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum MaterialType {
  Color = 0,
  VertexColor = 1,
  Textured = 2,
  CubeMap = 3,
}

pub struct Material {
  pub material_type: MaterialType,
  pub vertex_colors: Vec<[f32; 3]>,
  pub texture_coordinates: Vec<[f32; 2]>,
  pub texture_src: Vec<String>,
  pub color: Color,
}

impl Material {
  pub fn new(color: Color) -> Self {
    Self {
      material_type: MaterialType::Color,
      vertex_colors: vec![],
      texture_coordinates: vec![],
      texture_src: vec![],
      color,
    }
  }
  pub fn vertex_color(colors: Vec<[f32; 3]>) -> Self {
    Self {
      material_type: MaterialType::VertexColor,
      vertex_colors: colors,
      texture_coordinates: vec![],
      texture_src: vec![],
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
      texture_src: vec![src.to_string()],
      color: Color {
        r: 0.1,
        g: 0.1,
        b: 0.1,
        a: 1.,
      },
    }
  }
  pub fn cubemap(src_set: [&str; 6]) -> Self {
    Self {
      material_type: MaterialType::CubeMap,
      vertex_colors: vec![],
      texture_coordinates: vec![],
      texture_src: src_set.iter().map(|s| s.to_string()).collect(),
      color: Color {
        r: 0.,
        g: 0.,
        b: 0.,
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

  pub texture_coordinates: GpuBuffer,
  pub texture_bind_group: GpuBindGroup,
}

impl Mesh {
  pub async fn new(
    renderer: &Renderer,
    geometry: &Geometry,
    material: &Material,
  ) -> Result<Self, JsValue> {
    let device = renderer.device();
    let pipeline = if material.material_type == MaterialType::CubeMap {
      renderer.pipeline_cubebox()
    } else {
      renderer.pipeline()
    };
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

    let texture_bind_group = {
      let mut bitmaps = vec![];
      let mut rect = None;
      for each in material.texture_src.iter() {
        let (texture, r) = Renderer::create_bitmap(each).await?;
        if rect.is_none() {
          rect = Some(r);
        }
        bitmaps.push(texture);
      }
      let rect = rect.unwrap_or(Rect {
        width: 1,
        height: 1,
      });
      let texture = if material.material_type == MaterialType::CubeMap {
        renderer.create_texture(&rect, 6)
      } else {
        renderer.create_texture(&rect, 1)
      };
      for (i, bitmap) in bitmaps.into_iter().enumerate() {
        let mut source = GpuImageCopyExternalImage::new(&Object::new());
        source.flip_y(false);
        source.source(&Object::from(bitmap));
        let dest = if material.material_type == MaterialType::CubeMap {
          let mut dest = GpuImageCopyTextureTagged::new(&texture);
          dest.origin(&iter_to_array([0, 0, i as i32]));
          dest
        } else {
          GpuImageCopyTextureTagged::new(&texture)
        };
        device
          .queue()
          .copy_external_image_to_texture_with_u32_sequence(
            &source,
            &dest,
            &iter_to_array([rect.width, rect.height]),
          );
      }
      let mut entries = vec![JsValue::from(&GpuBindGroupEntry::new(
        0,
        &renderer.texture_sampler(),
      ))];
      if material.material_type == MaterialType::CubeMap {
        entries.push(JsValue::from(&GpuBindGroupEntry::new(
          1,
          &texture.create_view_with_descriptor(
            &GpuTextureViewDescriptor::new().dimension(GpuTextureViewDimension::Cube),
          ),
        )));
      } else {
        entries.push(JsValue::from(&GpuBindGroupEntry::new(
          1,
          &texture.create_view(),
        )));
      }
      let texture_binding_group =
        renderer
          .device()
          .create_bind_group(&GpuBindGroupDescriptor::new(
            &iter_to_array(&entries),
            &pipeline.get_bind_group_layout(1),
          ));
      texture_binding_group
    };

    let uniform_buffer = device.create_buffer(&GpuBufferDescriptor::new(
      96.,
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
    })
  }
}
