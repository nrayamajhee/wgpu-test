//! Camera-relative control of a rigid body. See [`Player`].

use std::cell::Cell;
use std::rc::Rc;

use fluid::add_event_and_forget;
use gloo_utils::window as gloo_window;
use nalgebra::{vector, UnitQuaternion, Vector3};
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

use crate::core::{Scene, Viewport};

/// Movement impulse level, toggled by holding Shift.
#[derive(Clone, Copy, PartialEq)]
pub enum MovementSpeed {
  /// Impulse 10 per call.
  Normal,
  /// Impulse 30 per call (Shift held).
  Fast,
}

impl MovementSpeed {
  /// Impulse magnitude for this speed.
  fn impulse(self) -> f32 {
    match self {
      MovementSpeed::Normal => 10.,
      MovementSpeed::Fast => 30.,
    }
  }
}

/// Drives the rigid body of scene node `node` relative to the camera.
///
/// Holds no scene/viewport references; pass them per call from the frame loop.
/// Methods return `None` if the node has no rigid body.
pub struct Player {
  /// Id of the scene node whose body is controlled.
  node: String,
  /// Current speed, updated by Shift listeners.
  speed: Rc<Cell<MovementSpeed>>,
}

impl Player {
  /// Controls node `node` and registers Shift listeners for [`MovementSpeed`].
  pub fn new(node: impl Into<String>) -> Self {
    let speed = Rc::new(Cell::new(MovementSpeed::Normal));
    for (event, value) in [
      ("keydown", MovementSpeed::Fast),
      ("keyup", MovementSpeed::Normal),
    ] {
      let speed = speed.clone();
      add_event_and_forget(&gloo_window(), event, move |e| {
        if e.dyn_into::<KeyboardEvent>().is_ok_and(|e| e.key() == "Shift") {
          speed.set(value);
        }
      });
    }
    Self {
      node: node.into(),
      speed,
    }
  }

  /// Current movement speed.
  pub fn speed(&self) -> MovementSpeed {
    self.speed.get()
  }

  /// Rotates the body to face the camera's XZ direction.
  pub fn face_viewport(&self, scene: &mut Scene, viewport: &Viewport) -> Option<()> {
    let facing = viewport.facing_xz();
    let forward = Vector3::new(facing.x, 0., facing.y);
    let rotation = UnitQuaternion::face_towards(&-forward, &Vector3::y());
    scene
      .get_body_mut(&self.node)?
      .set_rotation(rotation, true);
    Some(())
  }

  /// Applies a horizontal impulse: `dy` along camera forward, `dx` along
  /// camera right, scaled by [`MovementSpeed`].
  pub fn move_(&self, scene: &mut Scene, viewport: &Viewport, dx: isize, dy: isize) -> Option<()> {
    let facing = viewport.facing_xz();
    let forward = Vector3::new(facing.x, 0., facing.y);
    let right = Vector3::new(-facing.y, 0., facing.x);
    let magnitude = self.speed().impulse();
    let impulse = forward * (dy as f32 * magnitude) + right * (dx as f32 * magnitude);
    scene
      .get_body_mut(&self.node)?
      .apply_impulse(vector![impulse.x, 0., impulse.z], true);
    Some(())
  }
}
