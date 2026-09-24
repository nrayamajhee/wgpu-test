//! Binary glTF (`.glb`) import. See [`Geometry::from_gltf`].

use std::rc::Rc;

use gltf::mesh::util::ReadIndices;
use nalgebra::{Matrix4, Similarity, Similarity3, Translation3, UnitQuaternion, Vector3};

use crate::core::{Color, Geometry, Material, TextureSource};

/// Embedded buffers and images of a glTF file, indexed like the document.
struct Sources {
  /// Buffer contents (only the embedded BIN chunk is supported).
  buffers: Vec<Vec<u8>>,
  /// Encoded images. `Rc` so primitives sharing an image share one GPU upload.
  images: Vec<Rc<[u8]>>,
}

impl Geometry {
  /// Loads every mesh primitive from a binary glTF (`.glb`).
  ///
  /// Returns one `(geometry, material, transform)` per primitive, with node
  /// transforms flattened to model space. Uses the default scene (or all
  /// scenes).
  ///
  /// Reads positions, normals (computed if missing), tangents (computed if
  /// missing and a normal map is used), vertex colors, and the
  /// metallic-roughness material: base color, metallic-roughness, normal,
  /// occlusion and emissive textures with their factors.
  ///
  /// # Limitations
  /// - Only embedded buffers/images; external URIs load as empty.
  /// - One UV set per primitive (the base color texture's, else the normal
  ///   map's, else `TEXCOORD_0`).
  /// - Sampler settings and `alphaMode` are ignored (textures repeat, and
  ///   everything is opaque).
  ///
  /// # Errors
  /// If the data isn't valid glTF.
  pub fn from_gltf(data: &[u8]) -> Result<Vec<(Self, Material, Similarity3<f32>)>, gltf::Error> {
    let gltf::Gltf { document, blob } = gltf::Gltf::from_slice(data)?;

    let buffers: Vec<Vec<u8>> = document
      .buffers()
      .map(|buf| match buf.source() {
        gltf::buffer::Source::Bin => blob.clone().unwrap_or_default(),
        gltf::buffer::Source::Uri(_) => vec![],
      })
      .collect();
    let images = document
      .images()
      .map(|img| match img.source() {
        gltf::image::Source::View { view, .. } => {
          let buf = &buffers[view.buffer().index()];
          buf[view.offset()..view.offset() + view.length()].into()
        }
        gltf::image::Source::Uri { .. } => Rc::from([]),
      })
      .collect();
    let sources = Sources { buffers, images };

    let scenes: Vec<_> = document
      .default_scene()
      .map(|s| vec![s])
      .unwrap_or_else(|| document.scenes().collect());

    let mut out = Vec::new();
    for scene in scenes {
      for node in scene.nodes() {
        collect_node(&node, &Matrix4::identity(), &sources, &mut out);
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
  sources: &Sources,
  out: &mut Vec<(Geometry, Material, Similarity3<f32>)>,
) {
  let local = Matrix4::from_column_slice(node.transform().matrix().as_flattened());
  let transform = parent_transform * local;

  if let Some(mesh) = node.mesh() {
    let (t, r, s) = decompose_matrix(&transform);
    let similarity = Similarity::from_parts(t, r, s);
    for primitive in mesh.primitives() {
      if let Some((geometry, material)) = load_primitive(&primitive, sources) {
        out.push((geometry, material, similarity));
      }
    }
  }

  for child in node.children() {
    collect_node(&child, &transform, sources, out);
  }
}

/// Reads one primitive's vertex data and material. `None` if it has no positions.
fn load_primitive(primitive: &gltf::Primitive<'_>, sources: &Sources) -> Option<(Geometry, Material)> {
  let reader = primitive.reader(|buf| sources.buffers.get(buf.index()).map(|v| v.as_slice()));
  let vertices: Vec<[f32; 3]> = reader.read_positions()?.collect();
  let count = vertices.len();

  let indices = match reader.read_indices() {
    Some(ReadIndices::U8(iter)) => iter.map(u32::from).collect(),
    Some(ReadIndices::U16(iter)) => iter.map(u32::from).collect(),
    Some(ReadIndices::U32(iter)) => iter.collect(),
    None => (0..count as u32).collect(),
  };

  let gltf_material = primitive.material();
  let pbr = gltf_material.pbr_metallic_roughness();
  let normal_info = gltf_material.normal_texture();
  let texture = |index: usize| TextureSource::Bytes(sources.images[index].clone());

  let uv_set = pbr
    .base_color_texture()
    .map(|t| t.tex_coord())
    .or(normal_info.as_ref().map(|t| t.tex_coord()))
    .unwrap_or(0);
  let uvs: Vec<[f32; 2]> = reader
    .read_tex_coords(uv_set)
    .map(|tc| tc.into_f32().collect())
    .unwrap_or_default();

  let mut geometry = Geometry {
    vertices,
    normals: reader.read_normals().map(Iterator::collect).unwrap_or_default(),
    tangents: reader.read_tangents().map(Iterator::collect).unwrap_or_default(),
    indices,
  };
  if geometry.normals.len() != count {
    geometry.compute_normals();
  }
  if normal_info.is_some() && geometry.tangents.len() != count {
    geometry.compute_tangents(&uvs);
  }

  let [r, g, b, a] = pbr.base_color_factor();
  let mut material = Material::new(Color { r, g, b, a })
    .with_metallic(pbr.metallic_factor())
    .with_roughness(pbr.roughness_factor());
  material.texture_coordinates = uvs;
  material.vertex_colors = reader
    .read_colors(0)
    .map(|c| c.into_rgb_f32().collect())
    .unwrap_or_default();
  material.base_color_texture = pbr
    .base_color_texture()
    .map(|t| texture(t.texture().source().index()));
  if let Some(t) = pbr.metallic_roughness_texture() {
    material = material.with_metallic_roughness_texture(texture(t.texture().source().index()));
  }
  if let Some(t) = normal_info {
    material = material.with_normal_texture(texture(t.texture().source().index()), t.scale());
  }
  if let Some(t) = gltf_material.occlusion_texture() {
    material = material.with_occlusion_texture(texture(t.texture().source().index()), t.strength());
  }
  if let Some(t) = gltf_material.emissive_texture() {
    material = material.with_emissive_texture(texture(t.texture().source().index()));
  }
  let [r, g, b] = gltf_material.emissive_factor();
  material = material.with_emissive(Color::rgb(r, g, b));
  Some((geometry, material))
}

/// Splits an affine matrix into translation, rotation and a uniform scale
/// (average of the axis scales; non-uniform scale is approximated).
fn decompose_matrix(m: &Matrix4<f32>) -> (Translation3<f32>, UnitQuaternion<f32>, f32) {
  let translation = Translation3::new(m[(0, 3)], m[(1, 3)], m[(2, 3)]);
  let basis = m.fixed_view::<3, 3>(0, 0).into_owned();
  let scale = (0..3)
    .map(|c| Vector3::from(basis.column(c)).norm())
    .sum::<f32>()
    / 3.;
  let rotation = UnitQuaternion::from_matrix(&(basis / scale));
  (translation, rotation, scale)
}
