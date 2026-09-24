//! Surface descriptions. See [`Material`].

use std::rc::Rc;

use crate::core::Color;

/// Which pipeline draws a mesh.
#[derive(PartialEq, Clone, Copy, Debug)]
pub enum MaterialType {
  /// Lit glTF metallic-roughness shading (`pbr.wgsl`). The default.
  Pbr,
  /// Six-face cube texture, drawn as the skybox (`shader_cube.wgsl`). Also
  /// becomes the PBR environment reflected by every surface.
  CubeMap,
}

/// Where a texture's image comes from.
///
/// Uploaded textures are cached by source, so meshes sharing a source (same
/// URL, or clones of the same `Rc`) share one GPU texture.
#[derive(Clone)]
pub enum TextureSource {
  /// Image URL to fetch.
  Url(String),
  /// Encoded image bytes (PNG, JPEG, …), e.g. embedded in a glTF.
  Bytes(Rc<[u8]>),
}

/// Surface description consumed by [`Mesh::new`](crate::core::Mesh::new).
///
/// PBR values follow glTF metallic-roughness: each factor multiplies its
/// texture, and missing textures act as white (flat for normals). The
/// emissive factor defaults to black, so nothing glows unless set.
/// Colors are linear.
pub struct Material {
  /// Pipeline selection.
  pub material_type: MaterialType,
  /// Base color factor.
  pub color: Color,
  /// Metallic factor: `0` dielectric, `1` metal.
  pub metallic: f32,
  /// Roughness factor: `0` mirror, `1` fully diffuse.
  pub roughness: f32,
  /// Strength of the normal map's XY perturbation.
  pub normal_scale: f32,
  /// Per-vertex RGB multiplied into the base color (glTF `COLOR_0`).
  /// Empty = white.
  pub vertex_colors: Vec<[f32; 3]>,
  /// Per-vertex UVs shared by all textures. Empty = zeros.
  pub texture_coordinates: Vec<[f32; 2]>,
  /// Base color (albedo) texture, sRGB.
  pub base_color_texture: Option<TextureSource>,
  /// Tangent-space normal map, linear.
  pub normal_texture: Option<TextureSource>,
  /// Roughness in G and metallic in B (glTF packing), linear.
  pub metallic_roughness_texture: Option<TextureSource>,
  /// Ambient occlusion in R, linear. Darkens indirect (ambient and
  /// environment) light only.
  pub occlusion_texture: Option<TextureSource>,
  /// Occlusion effect: `0` none, `1` full.
  pub occlusion_strength: f32,
  /// Emitted light texture, sRGB. Multiplied by `emissive`.
  pub emissive_texture: Option<TextureSource>,
  /// Emissive factor (linear RGB, alpha unused); black = no emission.
  pub emissive: Color,
  /// Six face URLs (+x −x +y −y +z −z), for [`MaterialType::CubeMap`] only.
  pub cubemap_faces: Vec<String>,
}

impl Material {
  /// Plain dielectric of `color`, roughness 0.5.
  pub fn new(color: Color) -> Self {
    Self {
      material_type: MaterialType::Pbr,
      color,
      metallic: 0.,
      roughness: 0.5,
      normal_scale: 1.,
      vertex_colors: vec![],
      texture_coordinates: vec![],
      base_color_texture: None,
      normal_texture: None,
      metallic_roughness_texture: None,
      occlusion_texture: None,
      occlusion_strength: 1.,
      emissive_texture: None,
      emissive: Color::rgb(0., 0., 0.),
      cubemap_faces: vec![],
    }
  }

  /// Per-vertex color material; `colors` must match the geometry's vertex count.
  pub fn vertex_color(colors: Vec<[f32; 3]>) -> Self {
    Self {
      vertex_colors: colors,
      ..Self::new(Color::rgb(1., 1., 1.))
    }
  }

  /// Base color texture fetched from URL `src`, mapped with per-vertex UVs.
  pub fn textured(src: &str, coordinates: Vec<[f32; 2]>) -> Self {
    Self {
      texture_coordinates: coordinates,
      base_color_texture: Some(TextureSource::Url(src.to_owned())),
      ..Self::new(Color::rgb(1., 1., 1.))
    }
  }

  /// Base color texture decoded from `bytes`, multiplied by `color`.
  pub fn textured_bytes(bytes: impl Into<Rc<[u8]>>, coordinates: Vec<[f32; 2]>, color: Color) -> Self {
    Self {
      texture_coordinates: coordinates,
      base_color_texture: Some(TextureSource::Bytes(bytes.into())),
      ..Self::new(color)
    }
  }

  /// Cube map from six face URLs, ordered +x −x +y −y +z −z.
  pub fn cubemap(src_set: [&str; 6]) -> Self {
    Self {
      material_type: MaterialType::CubeMap,
      cubemap_faces: src_set.iter().map(|s| s.to_string()).collect(),
      ..Self::new(Color::rgb(1., 1., 1.))
    }
  }

  /// Sets the metallic factor.
  pub fn with_metallic(mut self, metallic: f32) -> Self {
    self.metallic = metallic;
    self
  }

  /// Sets the roughness factor.
  pub fn with_roughness(mut self, roughness: f32) -> Self {
    self.roughness = roughness;
    self
  }

  /// Sets the normal map and its strength.
  pub fn with_normal_texture(mut self, source: TextureSource, scale: f32) -> Self {
    self.normal_texture = Some(source);
    self.normal_scale = scale;
    self
  }

  /// Sets the metallic-roughness texture (roughness in G, metallic in B).
  pub fn with_metallic_roughness_texture(mut self, source: TextureSource) -> Self {
    self.metallic_roughness_texture = Some(source);
    self
  }

  /// Sets the occlusion texture (R channel) and its strength.
  pub fn with_occlusion_texture(mut self, source: TextureSource, strength: f32) -> Self {
    self.occlusion_texture = Some(source);
    self.occlusion_strength = strength;
    self
  }

  /// Sets the emissive factor, e.g. to make an untextured surface glow.
  pub fn with_emissive(mut self, emissive: Color) -> Self {
    self.emissive = emissive;
    self
  }

  /// Sets the emissive texture (multiplied by the emissive factor, which
  /// defaults to black, so set both).
  pub fn with_emissive_texture(mut self, source: TextureSource) -> Self {
    self.emissive_texture = Some(source);
    self
  }
}
