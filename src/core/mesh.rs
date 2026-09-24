//! CPU-side geometry/materials and their GPU-uploaded [`Mesh`] form.
//!
//! Flow: build a [`Geometry`] + [`Material`] → [`Mesh::new`] uploads them →
//! add the mesh to a [`Group`](crate::core::Group).

use crate::core::Rect;
use crate::core::Color;
use crate::{core::Renderer, utils::iter_to_array};
use genmesh::{
  generators::{IndexedPolygon, SharedVertex},
  EmitTriangles, Triangulate, Vertex,
};
use gltf::mesh::util::ReadIndices;
use js_sys::Object;
use nalgebra::{Matrix4, Similarity, Similarity3, Translation3, UnitQuaternion};
use wasm_bindgen::JsValue;
use web_sys::{
  gpu_buffer_usage, GpuBindGroup, GpuBindGroupDescriptor, GpuBindGroupEntry, GpuBuffer,
  GpuBufferBinding, GpuBufferDescriptor, GpuImageCopyExternalImage, GpuImageCopyTextureTagged,
  GpuTextureViewDescriptor, GpuTextureViewDimension,
};

/// Shading mode. The discriminant is sent to `shader.wgsl` as a uniform.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum MaterialType {
  /// Single flat color.
  Color = 0,
  /// Per-vertex colors.
  VertexColor = 1,
  /// 2D texture sampled with UVs, tinted by `color`.
  Textured = 2,
  /// Six-face cube texture; rendered with the skybox pipeline.
  CubeMap = 3,
}

/// Surface description consumed by [`Mesh::new`].
/// Only the fields relevant to `material_type` are used.
pub struct Material {
  /// Shading mode.
  pub material_type: MaterialType,
  /// One RGB per vertex ([`MaterialType::VertexColor`]).
  pub vertex_colors: Vec<[f32; 3]>,
  /// One UV per vertex ([`MaterialType::Textured`]).
  pub texture_coordinates: Vec<[f32; 2]>,
  /// Image URLs to fetch (1 for textured, 6 for cube map: +x −x +y −y +z −z).
  pub texture_src: Vec<String>,
  /// Encoded image bytes (e.g. PNG/JPEG embedded in a glTF).
  pub texture_bytes: Vec<Vec<u8>>,
  /// Base color, or tint for textures.
  pub color: Color,
}

impl Material {
  /// Flat color material.
  pub fn new(color: Color) -> Self {
    Self {
      material_type: MaterialType::Color,
      vertex_colors: vec![],
      texture_coordinates: vec![],
      texture_src: vec![],
      texture_bytes: vec![],
      color,
    }
  }
  /// Per-vertex color material; `colors` must match the geometry's vertex count.
  pub fn vertex_color(colors: Vec<[f32; 3]>) -> Self {
    Self {
      material_type: MaterialType::VertexColor,
      vertex_colors: colors,
      texture_coordinates: vec![],
      texture_src: vec![],
      texture_bytes: vec![],
      color: Color {
        r: 1.,
        g: 1.,
        b: 1.,
        a: 1.,
      },
    }
  }
  /// Texture fetched from URL `src`, mapped with per-vertex UVs.
  pub fn textured(src: &str, coordinates: Vec<[f32; 2]>) -> Self {
    Self {
      material_type: MaterialType::Textured,
      vertex_colors: vec![],
      texture_coordinates: coordinates,
      texture_src: vec![src.to_string()],
      texture_bytes: vec![],
      color: Color {
        r: 1.,
        g: 1.,
        b: 1.,
        a: 1.,
      },
    }
  }
  /// Texture decoded from encoded image `bytes`, tinted by `color`.
  pub fn textured_bytes(bytes: Vec<u8>, coordinates: Vec<[f32; 2]>, color: Color) -> Self {
    Self {
      material_type: MaterialType::Textured,
      vertex_colors: vec![],
      texture_coordinates: coordinates,
      texture_src: vec![],
      texture_bytes: vec![bytes],
      color,
    }
  }
  /// Cube map from six face URLs, ordered +x −x +y −y +z −z.
  pub fn cubemap(src_set: [&str; 6]) -> Self {
    Self {
      material_type: MaterialType::CubeMap,
      vertex_colors: vec![],
      texture_coordinates: vec![],
      texture_src: src_set.iter().map(|s| s.to_string()).collect(),
      texture_bytes: vec![],
      color: Color {
        r: 0.,
        g: 0.,
        b: 0.,
        a: 1.,
      },
    }
  }
}

/// Indexed triangle list on the CPU.
pub struct Geometry {
  /// Vertex positions.
  pub vertices: Vec<[f32; 3]>,
  /// Triangle indices into `vertices`, three per triangle, CCW front faces.
  pub indices: Vec<u32>,
}

impl Geometry {
  /// Triangulates any [`genmesh`] generator (e.g. `IcoSphere`, `Cube`).
  pub fn from_genmesh<T, P>(primitive: &T) -> Self
  where
    P: EmitTriangles<Vertex = usize>,
    T: SharedVertex<Vertex> + IndexedPolygon<P>,
  {
    let vertices = primitive
      .shared_vertex_iter()
      .map(|v| v.pos.into())
      .collect();
    let indices: Vec<u32> = primitive
      .indexed_polygon_iter()
      .triangulate()
      .flat_map(|i| [i.x as u32, i.y as u32, i.z as u32])
      .collect();
    Geometry { vertices, indices }
  }

  /// Box centred on the origin with `[width, height, depth]` along X, Y, Z.
  ///
  /// Use this for non-uniform shapes: [`Group`](crate::core::Group)
  /// transforms only support uniform scale.
  pub fn cuboid([width, height, depth]: [f32; 3]) -> Self {
    let mut geometry = Self::from_genmesh(&genmesh::generators::Cube::new());
    for v in geometry.vertices.iter_mut() {
      // Cube spans -1..1, so scale by half-extents.
      v[0] *= width / 2.;
      v[1] *= height / 2.;
      v[2] *= depth / 2.;
    }
    geometry
  }

  /// Square in the XZ plane, facing +Y, spanning `-half_size..half_size`.
  pub fn plane(half_size: f32) -> Self {
    let s = half_size;
    let vertices = vec![[-s, 0., -s], [s, 0., -s], [s, 0., s], [-s, 0., s]];
    let indices = vec![0, 2, 1, 0, 3, 2];
    Geometry { vertices, indices }
  }

  /// Loads every mesh primitive from a binary glTF (`.glb`).
  ///
  /// Returns one `(geometry, material, transform)` per primitive, with node
  /// transforms flattened to model space. Uses the default scene (or all
  /// scenes). Only embedded buffers/images are supported; external URIs load
  /// as empty. Materials use the PBR base color factor and texture only.
  ///
  /// # Errors
  /// If the data isn't valid glTF.
  pub fn from_gltf(data: &[u8]) -> Result<Vec<(Self, Material, Similarity3<f32>)>, gltf::Error> {
    let gltf::Gltf { document, blob } = gltf::Gltf::from_slice(data)?;

    let buffer_data: Vec<Vec<u8>> = document
      .buffers()
      .map(|buf| match buf.source() {
        gltf::buffer::Source::Bin => blob.clone().unwrap_or_default(),
        gltf::buffer::Source::Uri(_) => vec![],
      })
      .collect();

    let image_data: Vec<Vec<u8>> = document
      .images()
      .map(|img| match img.source() {
        gltf::image::Source::View { view, .. } => {
          let buf = &buffer_data[view.buffer().index()];
          buf[view.offset()..view.offset() + view.length()].to_vec()
        }
        gltf::image::Source::Uri { .. } => vec![],
      })
      .collect();

    let mut out: Vec<(Self, Material, Similarity3<f32>)> = Vec::new();

    let scenes: Vec<_> = document
      .default_scene()
      .map(|s| vec![s])
      .unwrap_or_else(|| document.scenes().collect());

    for scene in scenes {
      for node in scene.nodes() {
        collect_node(
          &node,
          &Matrix4::identity(),
          &buffer_data,
          &image_data,
          &mut out,
        );
      }
    }

    Ok(out)
  }
}

/// Recursively appends `node`'s primitives (and its children's) to `out`,
/// composing transforms with `parent_transform`.
fn collect_node(
  node: &gltf::Node<'_>,
  parent_transform: &Matrix4<f32>,
  buffer_data: &[Vec<u8>],
  image_data: &[Vec<u8>],
  out: &mut Vec<(Geometry, Material, Similarity3<f32>)>,
) {
  let local = Matrix4::from_column_slice(node.transform().matrix().as_flattened());
  let transform = parent_transform * local;

  if let Some(mesh) = node.mesh() {
    for primitive in mesh.primitives() {
      let reader = primitive.reader(|buf| buffer_data.get(buf.index()).map(|v| v.as_slice()));

      let positions: Vec<[f32; 3]> = match reader.read_positions() {
        Some(iter) => iter.collect(),
        None => continue,
      };

      let mut indices: Vec<u32> = Vec::new();
      match reader.read_indices() {
        Some(ReadIndices::U8(iter)) => indices.extend(iter.map(|i| i as u32)),
        Some(ReadIndices::U16(iter)) => indices.extend(iter.map(|i| i as u32)),
        Some(ReadIndices::U32(iter)) => indices.extend(iter),
        None => indices.extend(0..positions.len() as u32),
      }

      let pbr = primitive.material().pbr_metallic_roughness();
      let [r, g, b, a] = pbr.base_color_factor();
      let base_color = Color { r, g, b, a };

      let material = if let Some(tex_info) = pbr.base_color_texture() {
        let tex_coords: Vec<[f32; 2]> = reader
          .read_tex_coords(tex_info.tex_coord())
          .map(|tc| tc.into_f32().collect())
          .unwrap_or_default();
        let image_index = tex_info.texture().source().index();
        let bytes = image_data.get(image_index).cloned().unwrap_or_default();
        Material::textured_bytes(bytes, tex_coords, base_color)
      } else {
        Material::new(base_color)
      };

      let (t, r, s) = decompose_matrix(&transform);
      let similarity = Similarity::from_parts(t, r, s);
      out.push((
        Geometry {
          vertices: positions,
          indices,
        },
        material,
        similarity,
      ));
    }
  }

  for child in node.children() {
    collect_node(&child, &transform, buffer_data, image_data, out);
  }
}

/// Splits an affine matrix into translation, rotation and a uniform scale
/// (average of the axis scales; non-uniform scale is approximated).
fn decompose_matrix(m: &Matrix4<f32>) -> (Translation3<f32>, UnitQuaternion<f32>, f32) {
  let translation = Translation3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]);
  let scale = {
    let sx = nalgebra::Vector3::new(m[(0, 0)], m[(1, 0)], m[(2, 0)]).norm();
    let sy = nalgebra::Vector3::new(m[(0, 1)], m[(1, 1)], m[(2, 1)]).norm();
    let sz = nalgebra::Vector3::new(m[(0, 2)], m[(1, 2)], m[(2, 2)]).norm();
    (sx + sy + sz) / 3.0
  };
  let rot_mat = nalgebra::Matrix3::new(
    m[(0, 0)] / scale,
    m[(0, 1)] / scale,
    m[(0, 2)] / scale,
    m[(1, 0)] / scale,
    m[(1, 1)] / scale,
    m[(1, 2)] / scale,
    m[(2, 0)] / scale,
    m[(2, 1)] / scale,
    m[(2, 2)] / scale,
  );
  let rotation = UnitQuaternion::from_matrix(&rot_mat);
  (translation, rotation, scale)
}

/// GPU resources for one drawable: vertex/index buffers, per-mesh uniforms
/// and texture bindings. Unused buffers (e.g. colors on a textured mesh) are empty.
pub struct Mesh {
  /// Number of vertices.
  pub vertext_count: u32,
  /// Number of indices passed to `draw_indexed`.
  pub index_count: u32,
  /// Selects pipeline and shader branch.
  pub material_type: MaterialType,
  /// Base color / tint written to uniforms.
  pub color: Color,

  /// Positions, `vec3<f32>` per vertex (slot 0).
  pub vertex_buffer: GpuBuffer,
  /// `u32` triangle indices.
  pub index_buffer: GpuBuffer,
  /// Per-vertex RGB (slot 1, default pipeline).
  pub vertex_colors: GpuBuffer,

  /// 96 bytes: MVP matrix, color, material type (bind group 0).
  pub uniform_buffer: GpuBuffer,
  /// Binds `uniform_buffer`.
  pub uniform_bind_group: GpuBindGroup,

  /// Per-vertex UVs (slot 2, or slot 1 in the cube map pipeline).
  pub texture_coordinates: GpuBuffer,
  /// Sampler + texture view (bind group 1). A 1×1 placeholder if untextured.
  pub texture_bind_group: GpuBindGroup,
}

impl Mesh {
  /// Uploads `geometry` and `material` to the GPU, fetching/decoding any textures.
  ///
  /// # Errors
  /// If a texture fails to fetch or decode.
  pub async fn new(
    renderer: &Renderer,
    geometry: &Geometry,
    material: &Material,
  ) -> Result<Self, JsValue> {
    let device = renderer.device();
    let pipeline = if material.material_type == MaterialType::CubeMap {
      renderer.pipeline_cubemap()
    } else {
      renderer.pipeline()
    };
    let vertex_buffer = {
      let vertices: Vec<f32> = geometry.vertices.iter().flatten().copied().collect();
      renderer.create_buffer(&vertices)
    };
    let index_buffer = renderer.create_index_buffer(&geometry.indices[..]);
    let vertex_colors = if material.material_type == MaterialType::VertexColor {
      let vertices: Vec<f32> = material.vertex_colors.iter().flatten().copied().collect();
      renderer.create_buffer(&vertices)
    } else {
      renderer.create_buffer(&[])
    };
    let texture_coordinates = if material.material_type == MaterialType::Textured {
      let vertices: Vec<f32> = material
        .texture_coordinates
        .iter()
        .copied()
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
      for bytes in material.texture_bytes.iter() {
        let (texture, r) = Renderer::create_bitmap_from_bytes(bytes).await?;
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
        renderer.texture_sampler(),
      ))];
      if material.material_type == MaterialType::CubeMap {
        entries.push(JsValue::from(&GpuBindGroupEntry::new(
          1,
          &texture.create_view_with_descriptor(
            GpuTextureViewDescriptor::new().dimension(GpuTextureViewDimension::Cube),
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
