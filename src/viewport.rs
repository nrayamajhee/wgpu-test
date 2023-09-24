use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, Unit, UnitQuaternion, Vector3};
use std::f32::consts::PI;
use web_sys::HtmlCanvasElement;

pub struct Viewport {
  view: Isometry3<f32>,
  target: Isometry3<f32>,
  proj: Perspective3<f32>,
  zoom: bool,
  rotate: bool,
}

// const OPENGL_TO_WGPU: Matrix4<f32> = Matrix4::new(
//   1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
// );

impl Viewport {
  pub fn new(canvas: &HtmlCanvasElement) -> Self {
    let target = Isometry3::identity();
    let proj = Perspective3::new(
      canvas.width() as f32 / canvas.height() as f32,
      PI * 0.4,
      0.1,
      1000.,
    );
    let eye = [0., 2., 4.].into();
    let view = Isometry3::look_at_rh(&eye, &target.translation.vector.into(), &Vector3::y());
    Self {
      view,
      target,
      proj,
      zoom: false,
      rotate: false,
    }
  }
  pub fn view_proj(&self) -> Matrix4<f32> {
    self.proj.to_homogeneous() * self.view.to_homogeneous()
  }
  pub fn resize(&mut self, canvas: &HtmlCanvasElement) {
    self.proj = Perspective3::new(
      canvas.width() as f32 / canvas.height() as f32,
      PI / 3.,
      0.1,
      1000.,
    );
  }
  pub fn update_zoom(&mut self, ds: i32) {
    if self.zoom && ds != 0 {
      let delta = if ds > 0 { 1.05 } else { 0.95 };
      self.view.translation.vector = delta * self.view.translation.vector;
    }
  }
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
  pub fn unlock(&mut self) {
      self.zoom = true;
      self.rotate = true;
  }
  pub fn lock(&mut self) {
      self.zoom = true;
      self.rotate = true;
  }
}
