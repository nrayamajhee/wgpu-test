use nalgebra::Similarity3;
use web_sys::GpuBuffer;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Vertex {
  pub position: [f32; 3],
  pub tex_coords: [f32; 2],
}
pub struct Geometry {
  pub vertices: Vec<Vertex>,
  pub indices: Vec<u16>,
}

pub struct Mesh {
  pub vertex_buffer: GpuBuffer,
  pub index_buffer: GpuBuffer,
  pub num_indices: u32,
  pub model: Similarity3<f32>,
}

pub enum Primitive {
  Plane(Option<(usize, usize)>),
  Circle(Option<usize>),
}
