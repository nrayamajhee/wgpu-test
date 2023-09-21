use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, Unit, UnitQuaternion, Vector3};
use std::f32::consts::PI;
use web_sys::HtmlCanvasElement;

pub struct Viewport {
  view: Isometry3<f32>,
  target: Isometry3<f32>,
  proj: Perspective3<f32>,
}

// const OPENGL_TO_WGPU: Matrix4<f32> = Matrix4::new(
//   1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
// );

impl Viewport {
  pub fn new(canvas: &HtmlCanvasElement) -> Self {
    let target = Isometry3::identity();
    let proj = Perspective3::new(
      canvas.width() as f32 / canvas.height() as f32,
      PI / 2.,
      0.1,
      1000.,
    );
    let eye = [0., 2., 4.].into();
    let view = Isometry3::look_at_rh(&eye, &target.translation.vector.into(), &Vector3::y());
    Self { view, target, proj }
  }
  pub fn view_proj(&self) -> Matrix4<f32> {
    self.proj.to_homogeneous() * self.view.to_homogeneous()
  }
}
