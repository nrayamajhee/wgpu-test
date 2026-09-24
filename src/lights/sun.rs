//! See [`Sun`].

use nalgebra::{Unit, Vector3};

use crate::core::Color;

/// Directional light: parallel rays with no falloff, like sunlight.
pub struct Sun {
  /// Direction the light travels, in the node's local space.
  pub direction: Unit<Vector3<f32>>,
  /// Linear RGB color.
  pub color: Color,
  /// Multiplier on `color` (irradiance on a surface facing the sun).
  pub intensity: f32,
}

impl Sun {
  /// Sun shining along `direction` (normalized here).
  pub fn new(direction: Vector3<f32>, color: Color, intensity: f32) -> Self {
    Self {
      direction: Unit::new_normalize(direction),
      color,
      intensity,
    }
  }
}
