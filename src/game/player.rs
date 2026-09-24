//! Camera-relative control of a rigid body. See [`Player`].

use nalgebra::{vector, UnitQuaternion, Vector3};

use crate::core::{InputState, Scene, Viewport};

/// Movement impulse level: Shift held = fast.
#[derive(Clone, Copy, PartialEq)]
pub enum MovementSpeed {
  /// Impulse 10 per frame.
  Normal,
  /// Impulse 30 per frame (Shift held).
  Fast,
}

impl MovementSpeed {
  /// Speed selected by `input` (Shift → fast).
  fn from_input(input: &InputState) -> Self {
    if input.shift() {
      MovementSpeed::Fast
    } else {
      MovementSpeed::Normal
    }
  }

  /// Impulse magnitude for this speed.
  fn impulse(self) -> f32 {
    match self {
      MovementSpeed::Normal => 10.,
      MovementSpeed::Fast => 30.,
    }
  }
}

/// Drives the rigid body of scene node `node` from WASD relative to the camera.
///
/// Stateless apart from the node id: call [`Player::update`] every frame.
pub struct Player {
  /// Id of the scene node whose body is controlled.
  node: String,
}

impl Player {
  /// Controls node `node`.
  pub fn new(node: impl Into<String>) -> Self {
    Self { node: node.into() }
  }

  /// Faces the body along the camera and applies WASD movement.
  /// Returns `None` if the node has no rigid body.
  pub fn update(&self, scene: &mut Scene, viewport: &Viewport, input: &InputState) -> Option<()> {
    let axis = |plus, minus| (input.is_held(plus) as i8 - input.is_held(minus) as i8) as f32;
    let (dx, dy) = (axis("KeyD", "KeyA"), axis("KeyW", "KeyS"));
    let facing = viewport.facing_xz();
    let forward = Vector3::new(facing.x, 0., facing.y);
    let right = Vector3::new(-facing.y, 0., facing.x);
    let impulse = (forward * dy + right * dx) * MovementSpeed::from_input(input).impulse();

    let body = scene.get_body_mut(&self.node)?;
    body.set_rotation(UnitQuaternion::face_towards(&-forward, &Vector3::y()), true);
    if dx != 0. || dy != 0. {
      body.apply_impulse(vector![impulse.x, 0., impulse.z], true);
    }
    Some(())
  }
}
