//! Light sources. Attach one to a scene node with
//! [`Group::with_light`](crate::core::Group::with_light); the renderer
//! gathers them every frame.
//!
//! | Type | Placement from its node |
//! |---|---|
//! | [`Ambient`] | none (uniform) |
//! | [`Sun`] | direction is rotated by the node's world rotation |
//! | [`PointLight`] | position is the node's world translation |

mod ambient;
mod point;
mod sun;

pub use ambient::Ambient;
pub use point::PointLight;
pub use sun::Sun;

/// Most suns the PBR shader evaluates; extras are ignored.
/// Must match `MAX_SUNS` in `pbr.wgsl`.
pub const MAX_SUNS: usize = 4;

/// Most point lights the PBR shader evaluates; extras are ignored.
/// Must match `MAX_POINT_LIGHTS` in `pbr.wgsl`.
pub const MAX_POINT_LIGHTS: usize = 8;

/// Any light a scene node can carry.
pub enum Light {
  /// Uniform light from all directions.
  Ambient(Ambient),
  /// Parallel light from infinitely far away.
  Sun(Sun),
  /// Light radiating from a point.
  Point(PointLight),
}

impl From<Ambient> for Light {
  fn from(light: Ambient) -> Self {
    Light::Ambient(light)
  }
}

impl From<Sun> for Light {
  fn from(light: Sun) -> Self {
    Light::Sun(light)
  }
}

impl From<PointLight> for Light {
  fn from(light: PointLight) -> Self {
    Light::Point(light)
  }
}
