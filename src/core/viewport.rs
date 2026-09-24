//! Orbit camera. See [`Viewport`].

use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, Unit, UnitQuaternion, Vector2, Vector3};
use std::f32::consts::PI;
use web_sys::HtmlCanvasElement;

/// Perspective camera orbiting a target.
///
/// The final view is `proj * view * target⁻¹`: `target` places the orbit
/// center in the world and `view` is the eye's offset/orientation around it.
/// Zoom and rotation input is ignored while [locked](Viewport::lock).
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
  /// Whether wheel zoom is accepted.
  zoom: bool,
  /// Whether mouse rotation is accepted.
  rotate: bool,
}

impl Viewport {
  /// Vertical field of view: 72°.
  const FOV: f32 = PI * 0.4;
  /// Near clip plane distance.
  const NEAR: f32 = 0.1;
  /// Far clip plane distance (the skybox is drawn at infinity regardless).
  const FAR: f32 = 100000.;

  /// Perspective projection for `canvas`'s aspect ratio.
  fn projection(canvas: &HtmlCanvasElement) -> Perspective3<f32> {
    let aspect = canvas.width() as f32 / canvas.height() as f32;
    Perspective3::new(aspect, Self::FOV, Self::NEAR, Self::FAR)
  }

  /// Camera 10 units from the origin looking at it, locked, with no
  /// distance limits.
  pub fn new(canvas: &HtmlCanvasElement) -> Self {
    let target = Isometry3::identity();
    let proj = Self::projection(canvas);
    let eye = [0., 0., 10.].into();
    let view = Isometry3::look_at_rh(&eye, &target.translation.vector.into(), &Vector3::y());
    Self {
      view,
      target,
      proj,
      min_distance: 0.,
      max_distance: f32::INFINITY,
      zoom: false,
      rotate: false,
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

  /// Rebuilds the projection for the canvas's new aspect ratio.
  pub fn resize(&mut self, canvas: &HtmlCanvasElement) {
    self.proj = Self::projection(canvas);
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

  /// Zooms by 5% per wheel event: `ds > 0` zooms out, `ds < 0` zooms in.
  /// Clamped to the min/max distance.
  pub fn update_zoom(&mut self, ds: i32) {
    if self.zoom && ds != 0 {
      let delta = if ds > 0 { 1.05 } else { 0.95 };
      self.view.translation.vector = delta * self.view.translation.vector;
      self.clamp_distance();
    }
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
  /// X axis, `dx` yaws about the target's Y axis. `_dt` is unused.
  pub fn update_rot(&mut self, dx: i32, dy: i32, _dt: f32) {
    if self.rotate {
      let pitch = dy as f32 * 0.002;
      let yaw = dx as f32 * 0.002;
      let delta_rot = {
        let axis = Unit::new_normalize(self.view.rotation.conjugate() * Vector3::x());
        let q_ver = UnitQuaternion::from_axis_angle(&axis, pitch);
        let axis = Unit::new_normalize(self.target.rotation.conjugate() * Vector3::y());
        let q_hor = UnitQuaternion::from_axis_angle(&axis, yaw);
        q_ver * q_hor
      };
      self.view.rotation *= &delta_rot;
    }
  }

  /// Enables zoom and rotation input.
  pub fn unlock(&mut self) {
    self.zoom = true;
    self.rotate = true;
  }

  /// Disables zoom and rotation input.
  pub fn lock(&mut self) {
    self.zoom = false;
    self.rotate = false;
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
