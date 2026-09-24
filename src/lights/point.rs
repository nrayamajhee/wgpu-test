//! See [`PointLight`].

use crate::core::Color;

/// Omnidirectional light at its node's position.
///
/// Falloff follows glTF `KHR_lights_punctual`: inverse-square, smoothly
/// windowed to zero at `range`.
pub struct PointLight {
  /// Linear RGB color.
  pub color: Color,
  /// Multiplier on `color` (irradiance at 1 unit distance).
  pub intensity: f32,
  /// Distance at which the light fades out completely.
  pub range: f32,
}

impl PointLight {
  /// Point light of `color` × `intensity`, reaching `range` units.
  pub fn new(color: Color, intensity: f32, range: f32) -> Self {
    Self {
      color,
      intensity,
      range,
    }
  }
}
