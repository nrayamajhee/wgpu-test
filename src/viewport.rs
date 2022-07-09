use crate::renderer::get_window_dimension;
use crate::start::Events;
use nalgebra::{Isometry3, Matrix4, Perspective3, Point3, Unit, UnitQuaternion, Vector3};
use std::f32::consts::PI;

pub struct Viewport {
  view: Isometry3<f32>,
  target: Isometry3<f32>,
  proj: Perspective3<f32>,
}

impl Viewport {
  pub fn new() -> Self {
    let (width, height) = get_window_dimension();
    let mut target = Isometry3::identity();
    target.translation.vector = [0., 0., 1.].into();
    let proj = Perspective3::new(width as f32 / height as f32, PI / 2., 0.1, 1000.);
    let eye = [0., 0., 2.].into();
    let mut target = Isometry3::identity();
    let target_pos = [0., 0., 0.];
    target.translation = target_pos.into();
    let view = Isometry3::look_at_rh(&eye, &target_pos.into(), &Vector3::y());
    Self { view, target, proj }
  }
  pub fn update(&mut self, events: &Events, dt: f64) {
    let forward = self.target.translation.vector - self.view.translation.vector;
    let forward_norm = forward.normalize();
    let forward_magnitude = forward_norm.magnitude();

    if events.mouse.ds != 0. {
      //log::info!("{:?}{:?}{:?}",forward, right, self.target - (forward + right * 1.).normalize());
      let speed = 0.1;
      let dir = if events.mouse.ds > 0. { -1. } else { 1. };
      //self.view += forward_norm * speed * dir;
    }

    //let right = forward_norm.cross(&self.up);
    //let forward = self.target - self.eye;
    //let forward_magnitude = forward_norm.magnitude();

    let speed = 0.0001;
    let axis = self.view.rotation * Vector3::y_axis();
    let d_rad = speed * events.mouse.dx as f32 * dt as f32;
    let q_hor = UnitQuaternion::from_axis_angle(&axis, d_rad);
    let axis = self.view.rotation * Vector3::x_axis();
    let d_rad = speed * events.mouse.dy as f32 * dt as f32;
    let q_ver = UnitQuaternion::from_axis_angle(&axis, d_rad);
    let delta_rot = q_hor * q_ver;

    {
      self.view.rotation *= delta_rot;
      //self.view.translation.vector -= self.target.translation.vector;
      //self.view.translation.vector = self.view.rotation * Vector3::new(0.,0.,2.);
      //+ v.translation.vector;
    }

    if events.mouse.dx != 0 {
      //let dir = if events.mouse.dx > 0 { -1. } else { 1. };
      //self.eye = self.target - (forward + right * dir * speed).normalize() * forward_magnitude;
    }
  }
  pub fn view_proj(&self) -> Matrix4<f32> {
    //Matrix4::from(self.proj) * Matrix4::look_at_rh(&Point3::from_slice(self.view.translation.vector.as_slice()), &Point3::from(self.target.translation.vector.as_slice()), &(self.view.rotation * Vector3::y()))
    Matrix4::from(self.proj) * self.view.to_homogeneous()
  }
  pub fn resize(&mut self) {
    let (width, height) = get_window_dimension();
    self.proj = Perspective3::new(width as f32 / height as f32, PI / 2., 0.1, 1000.);
  }
}
