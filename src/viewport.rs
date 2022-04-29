use nalgebra::{Isometry3, {Vector3, Matrix4}, Perspective3};
use std::f32::consts::PI;

pub struct Viewport {
    view: Isometry3<f32>,
    proj: Perspective3<f32>,
    target: Isometry3<f32>,
}

impl Viewport {
    pub fn new() ->  Self {
        let (width, height) = crate::get_window_dimension(); 
        let mut target = Isometry3::identity();
        target.translation.vector = [0.,0.,1.].into();
        let proj = Perspective3::new(
            width as f32 / height as f32,
            PI / 2.,
            0.1,
            1000.
        );
        let view = Isometry3::look_at_rh(
            &target.translation.vector.into(),
            &[0., 0., 0.].into(),
            &Vector3::y(),
        );
        Self {
            view,
            target,
            proj,
        }
    }
    pub fn view_proj(&self) -> Matrix4<f32> {
        let view = self.view.to_homogeneous() * self.target.inverse().to_homogeneous();
        let lh_projection = Matrix4::new(
            1.0, 0.0, 0.0, 0.0,
            0.0, 1.0, 0.0, 0.0,
            0.0, 0.0, 0.5, 0.0,
            0.0, 0.0, 0.5, 1.0,
        );
        lh_projection * Matrix4::from(self.proj) * view
    }
}

