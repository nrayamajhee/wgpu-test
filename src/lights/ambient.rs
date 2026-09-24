//! See [`Ambient`].

use crate::core::Color;

/// Constant light added to every surface's diffuse term, regardless of
/// direction. Multiple ambients add up.
pub struct Ambient {
  /// Linear RGB color.
  pub color: Color,
  /// Multiplier on `color`.
  pub intensity: f32,
}

impl Ambient {
  /// Ambient light of `color` scaled by `intensity`.
  pub fn new(color: Color, intensity: f32) -> Self {
    Self { color, intensity }
  }
}
