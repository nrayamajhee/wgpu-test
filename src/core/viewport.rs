//! Orbit camera. See [`Viewport`].

use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, Unit, UnitQuaternion, Vector2, Vector3};
use std::f32::consts::PI;
use web_sys::HtmlCanvasElement;

use crate::core::InputState;

/// Perspective camera orbiting a target.
///
/// The final view is `proj * view * target⁻¹`: `target` places the orbit
/// center in the world and `view` is the eye's offset/orientation around it.
/// Owned by the frame loop and driven by [`Viewport::update`].
pub struct Viewport {
  /// Eye transform relative to the target; its translation length is the orbit distance.
  view: Isometry3<f32>,
  /// World-space orbit center, updated by [`Viewport::follow`].
  target: Isometry3<f32>,
  /// Perspective projection.
  proj: Perspective3<f32>,
  /// Closest allowed orbit distance.
  min_distance: f32,
  /// Farthest allowed orbit distance.
  max_distance: f32,
  /// Size the projection was built for; see [`Viewport::fit`].
  size: (u32, u32),
}

impl Viewport {
  /// Vertical field of view: 72°.
  const FOV: f32 = PI * 0.4;
  /// Near clip plane distance.
  const NEAR: f32 = 0.1;
  /// Far clip plane distance (the skybox is drawn at infinity regardless).
  const FAR: f32 = 100000.;

  /// Perspective projection for a `(width, height)` viewport.
  fn projection((width, height): (u32, u32)) -> Perspective3<f32> {
    let aspect = width.max(1) as f32 / height.max(1) as f32;
    Perspective3::new(aspect, Self::FOV, Self::NEAR, Self::FAR)
  }

  /// Camera 10 units from the origin looking at it, with no distance limits.
  pub fn new(canvas: &HtmlCanvasElement) -> Self {
    let target = Isometry3::identity();
    let size = (canvas.width(), canvas.height());
    let proj = Self::projection(size);
    let eye = [0., 0., 10.].into();
    let view = Isometry3::look_at_rh(&eye, &target.translation.vector.into(), &Vector3::y());
    Self {
      view,
      target,
      proj,
      min_distance: 0.,
      max_distance: f32::INFINITY,
      size,
    }
  }

  /// Moves the orbit center to `target` (call every frame to track something).
  pub fn follow(&mut self, target: Isometry3<f32>) {
    self.target = target;
  }

  /// Projection × camera rotation only, for the skybox (no translation).
  pub fn view_cube(&self) -> Matrix4<f32> {
    self.proj.to_homogeneous() * self.view.rotation.to_homogeneous()
  }

  /// Camera position in world space (for view-dependent shading).
  pub fn eye_position(&self) -> Point3<f32> {
    (self.target * self.view.inverse()).translation.vector.into()
  }

  /// Full world → clip-space matrix.
  pub fn view_proj(&self) -> Matrix4<f32> {
    self.proj.to_homogeneous() * self.view.to_homogeneous() * self.target.inverse().to_homogeneous()
  }

  /// Rebuilds the projection if `size` differs from the last one. Call every
  /// frame with [`AppState::size`](crate::core::AppState::size).
  pub fn fit(&mut self, size: (u32, u32)) {
    if size != self.size {
      self.size = size;
      self.proj = Self::projection(size);
    }
  }

  /// Caps how close the camera can zoom in to its target.
  pub fn set_min_distance(&mut self, min_distance: f32) {
    self.min_distance = min_distance;
    self.clamp_distance();
  }

  /// Caps how far the camera can zoom out from its target.
  pub fn set_max_distance(&mut self, max_distance: f32) {
    self.max_distance = max_distance;
    self.clamp_distance();
  }

  /// Applies this frame's mouse input: movement orbits, wheel zooms.
  /// Ignored while `paused`.
  pub fn update(&mut self, input: &InputState, paused: bool) {
    if paused {
      return;
    }
    let (dx, dy) = input.mouse_delta();
    if dx != 0 || dy != 0 {
      self.orbit(dx, dy);
    }
    let wheel = input.wheel();
    if wheel != 0. {
      self.zoom(wheel);
    }
  }

  /// Zooms 5% per frame with scroll: `wheel > 0` out, `< 0` in. Clamped to
  /// the min/max distance.
  fn zoom(&mut self, wheel: f32) {
    let factor = if wheel > 0. { 1.05 } else { 0.95 };
    self.view.translation.vector *= factor;
    self.clamp_distance();
  }

  /// Rescales the eye offset into `min_distance..=max_distance`.
  fn clamp_distance(&mut self) {
    let offset = &mut self.view.translation.vector;
    let distance = offset.norm();
    if distance <= f32::EPSILON {
      return;
    }
    let clamped = distance.clamp(self.min_distance, self.max_distance);
    if clamped != distance {
      *offset *= clamped / distance;
    }
  }

  /// Orbits by mouse movement in pixels: `dy` pitches about the camera's
  /// X axis, `dx` yaws about the target's Y axis.
  fn orbit(&mut self, dx: i32, dy: i32) {
    let pitch = dy as f32 * 0.002;
    let yaw = dx as f32 * 0.002;
    let axis = Unit::new_normalize(self.view.rotation.conjugate() * Vector3::x());
    let q_ver = UnitQuaternion::from_axis_angle(&axis, pitch);
    let axis = Unit::new_normalize(self.target.rotation.conjugate() * Vector3::y());
    let q_hor = UnitQuaternion::from_axis_angle(&axis, yaw);
    self.view.rotation *= q_ver * q_hor;
  }

  /// Camera forward direction projected onto the XZ plane, normalized.
  /// Falls back to `(0, -1)` when looking straight up/down.
  pub fn facing_xz(&self) -> Vector2<f32> {
    let forward_world = self.view.rotation.inverse() * (-Vector3::z());
    let xz = Vector2::new(forward_world.x, forward_world.z);
    if xz.norm() < 1e-6 {
      Vector2::new(0., -1.)
    } else {
      xz.normalize()
    }
  }
}
