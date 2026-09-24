//! GPU-uploaded drawables. See [`Mesh`].
//!
//! Flow: build a [`Geometry`] + [`Material`] → [`Mesh::new`] uploads them →
//! add the mesh to a [`Group`](crate::core::Group).

use wasm_bindgen::JsValue;
use web_sys::{
  GpuBindGroup, GpuBindGroupEntry, GpuBuffer, GpuBufferBinding, GpuTexture,
  GpuTextureViewDescriptor, GpuTextureViewDimension,
};

use crate::core::{Color, Geometry, Material, MaterialType, Renderer};

/// Size of a PBR mesh's uniform block (`Object` in `pbr.wgsl`): model
/// matrix, base color, `[metallic, roughness, normal_scale, occlusion_strength]`
/// and emissive.
pub const PBR_UNIFORM_SIZE: f64 = 112.;
/// Size of a cube map mesh's uniform block: one `mat4x4<f32>`.
pub const CUBEMAP_UNIFORM_SIZE: f64 = 64.;

/// Default texel for a missing base color, metallic-roughness, occlusion or
/// emissive texture (emissive is gated by its black default factor).
const WHITE: [u8; 4] = [255, 255, 255, 255];
/// Default texel for a missing normal map: +Z, i.e. unperturbed.
const FLAT_NORMAL: [u8; 4] = [128, 128, 255, 255];

/// GPU resources for one drawable.
///
/// # Buffers and bind groups by pipeline
/// | | PBR | Cube map |
/// |---|---|---|
/// | `vertex_buffers` | position, normal, tangent, UV, color (slots 0–4) | position (slot 0) |
/// | `bind_groups` | `[group 1]`: uniforms, sampler, base color, normal, metallic-roughness, occlusion, emissive | `[group 0, group 1]`: uniforms; sampler + cube |
///
/// PBR group 0 (camera, lights, environment) is owned by the [`Renderer`].
/// Every vertex buffer is full-length; missing data is filled with defaults.
pub struct Mesh {
  /// Number of vertices.
  pub vertext_count: u32,
  /// Number of indices passed to `draw_indexed`.
  pub index_count: u32,
  /// Selects the pipeline.
  pub material_type: MaterialType,
  /// Base color factor, written to uniforms every frame (mutable at runtime).
  pub color: Color,
  /// Metallic factor, written to uniforms every frame.
  pub metallic: f32,
  /// Roughness factor, written to uniforms every frame.
  pub roughness: f32,
  /// Normal map strength, written to uniforms every frame.
  pub normal_scale: f32,
  /// Occlusion strength, written to uniforms every frame.
  pub occlusion_strength: f32,
  /// Emissive factor, written to uniforms every frame.
  pub emissive: Color,
  /// Vertex buffers in slot order (see table above).
  pub vertex_buffers: Vec<GpuBuffer>,
  /// `u32` triangle indices.
  pub index_buffer: GpuBuffer,
  /// Per-mesh uniforms, rewritten by the renderer each frame.
  pub uniform_buffer: GpuBuffer,
  /// Mesh-owned bind groups (see table above).
  pub bind_groups: Vec<GpuBindGroup>,
  /// For cube maps: the cube texture, used by the renderer as the PBR
  /// environment.
  pub environment: Option<GpuTexture>,
}

impl Mesh {
  /// Uploads `geometry` and `material` to the GPU, fetching/decoding any
  /// textures (shared via the renderer's texture cache).
  ///
  /// # Errors
  /// If a texture fails to fetch or decode.
  pub async fn new(
    renderer: &Renderer,
    geometry: &Geometry,
    material: &Material,
  ) -> Result<Self, JsValue> {
    let count = geometry.vertices.len();
    let positions = renderer.create_buffer(&flatten(&geometry.vertices));
    let index_buffer = renderer.create_index_buffer(&geometry.indices);

    let (vertex_buffers, uniform_buffer, bind_groups, environment) = match material.material_type {
      MaterialType::CubeMap => {
        let texture = renderer.cubemap_texture(&material.cubemap_faces).await?;
        let uniform_buffer = renderer.create_uniform_buffer(CUBEMAP_UNIFORM_SIZE);
        let layout = |i| renderer.pipeline_cubemap().get_bind_group_layout(i);
        let uniforms = renderer.create_bind_group(
          &layout(0),
          &[GpuBindGroupEntry::new(0, &GpuBufferBinding::new(&uniform_buffer))],
        );
        let cube_view = texture.create_view_with_descriptor(
          GpuTextureViewDescriptor::new().dimension(GpuTextureViewDimension::Cube),
        );
        let textures = renderer.create_bind_group(
          &layout(1),
          &[
            GpuBindGroupEntry::new(0, renderer.texture_sampler()),
            GpuBindGroupEntry::new(1, &cube_view),
          ],
        );
        (
          vec![positions],
          uniform_buffer,
          vec![uniforms, textures],
          Some(texture),
        )
      }
      MaterialType::Pbr => {
        let normals = if geometry.normals.len() == count {
          flatten(&geometry.normals)
        } else {
          let mut computed = Geometry {
            vertices: geometry.vertices.clone(),
            normals: vec![],
            tangents: vec![],
            indices: geometry.indices.clone(),
          };
          computed.compute_normals();
          flatten(&computed.normals)
        };
        let vertex_buffers = vec![
          positions,
          renderer.create_buffer(&normals),
          renderer.create_buffer(&filled(&geometry.tangents, count, [0.; 4])),
          renderer.create_buffer(&filled(&material.texture_coordinates, count, [0.; 2])),
          renderer.create_buffer(&filled(&material.vertex_colors, count, [1.; 3])),
        ];

        let texture = |source, srgb, default| async move {
          match source {
            Some(source) => renderer.texture(source, srgb).await,
            None => Ok(renderer.solid_texture(default, srgb)),
          }
        };
        let base_color = texture(material.base_color_texture.as_ref(), true, WHITE).await?;
        let normal = texture(material.normal_texture.as_ref(), false, FLAT_NORMAL).await?;
        let metallic_roughness =
          texture(material.metallic_roughness_texture.as_ref(), false, WHITE).await?;
        let occlusion = texture(material.occlusion_texture.as_ref(), false, WHITE).await?;
        let emissive = texture(material.emissive_texture.as_ref(), true, WHITE).await?;

        let uniform_buffer = renderer.create_uniform_buffer(PBR_UNIFORM_SIZE);
        let bind_group = renderer.create_bind_group(
          &renderer.pipeline_pbr().get_bind_group_layout(1),
          &[
            GpuBindGroupEntry::new(0, &GpuBufferBinding::new(&uniform_buffer)),
            GpuBindGroupEntry::new(1, renderer.texture_sampler()),
            GpuBindGroupEntry::new(2, &base_color.create_view()),
            GpuBindGroupEntry::new(3, &normal.create_view()),
            GpuBindGroupEntry::new(4, &metallic_roughness.create_view()),
            GpuBindGroupEntry::new(5, &occlusion.create_view()),
            GpuBindGroupEntry::new(6, &emissive.create_view()),
          ],
        );
        (vertex_buffers, uniform_buffer, vec![bind_group], None)
      }
    };

    Ok(Self {
      vertext_count: count as u32,
      index_count: geometry.indices.len() as u32,
      material_type: material.material_type,
      color: material.color,
      metallic: material.metallic,
      roughness: material.roughness,
      normal_scale: material.normal_scale,
      occlusion_strength: material.occlusion_strength,
      emissive: material.emissive,
      vertex_buffers,
      index_buffer,
      uniform_buffer,
      bind_groups,
      environment,
    })
  }
}

/// Flattens `[f32; N]` items into a plain `f32` list for upload.
fn flatten<const N: usize>(items: &[[f32; N]]) -> Vec<f32> {
  items.iter().flatten().copied().collect()
}

/// Flattens `items` if it has one entry per vertex, else `count` × `default`.
fn filled<const N: usize>(items: &[[f32; N]], count: usize, default: [f32; N]) -> Vec<f32> {
  if items.len() == count {
    flatten(items)
  } else {
    flatten(&vec![default; count])
  }
}
