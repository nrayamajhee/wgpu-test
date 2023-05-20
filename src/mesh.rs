use genmesh::{
  generators::{Circle, Cube, IndexedPolygon, Plane, SharedVertex},
  EmitTriangles, Triangulate, Vertex,
};
use js_sys::{Float32Array, WebAssembly};
use wasm_bindgen::JsCast;
use web_sys::{gpu_buffer_usage, GpuBuffer, GpuBufferDescriptor, GpuDevice};

pub enum Primitive {
  Plane(Option<(usize, usize)>),
  Circle(Option<usize>),
  Cube,
}

pub struct Geometry {
  pub vertices: Vec<[f32; 3]>,
  pub indices: Vec<u16>,
}

impl Geometry {
  pub fn from_genmesh<T, P>(primitive: &T) -> Self
  where
    P: EmitTriangles<Vertex = usize>,
    T: SharedVertex<Vertex> + IndexedPolygon<P>,
  {
    let vertices = primitive
      .shared_vertex_iter()
      .map(|v| v.pos.into())
      .collect();
    let indices: Vec<u16> = primitive
      .indexed_polygon_iter()
      .triangulate()
      .flat_map(|i| [i.x as u16, i.y as u16, i.z as u16])
      .collect();
    Geometry { vertices, indices }
  }
  pub fn from_primitive(primitive: Primitive) -> Self {
    match primitive {
      Primitive::Plane(subdivision) => {
        let (x, y) = subdivision.unwrap_or((1, 1));
        Self::from_genmesh(&Plane::subdivide(x, y))
      }
      Primitive::Cube => Self::from_genmesh(&Cube::new()),
      Primitive::Circle(subdivision) => {
        let s = subdivision.unwrap_or(4);
        Self::from_genmesh(&Circle::new(s))
      }
    }
  }
}

pub struct Mesh {
  pub vertext_count: u32,
  pub vertex_buffer: GpuBuffer,
  pub index_buffer: GpuBuffer,
}

impl Mesh {
  pub fn from_geometry(device: &GpuDevice, geometry: &Geometry) -> Self {
    let size = geometry.vertices.len() * 3 * 4;
    let vertex_buffer = device.create_buffer(
      &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
    );
    let memory_buffer = wasm_bindgen::memory()
      .dyn_into::<WebAssembly::Memory>()
      .unwrap()
      .buffer();
    let array =
      Float32Array::new_with_byte_offset(&memory_buffer, geometry.vertices.as_ptr() as u32);
    let write_array = Float32Array::new(&vertex_buffer.get_mapped_range());
    write_array.set(&array, 0);
    vertex_buffer.unmap();
    let size = geometry.indices.len() * 3 * 4;
    let index_buffer = device.create_buffer(
      &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::INDEX).mapped_at_creation(true),
    );
    let array =
      Float32Array::new_with_byte_offset(&memory_buffer, geometry.indices.as_ptr() as u32);
    let write_array = Float32Array::new(&index_buffer.get_mapped_range());
    write_array.set(&array, 0);
    index_buffer.unmap();
    Self {
      vertext_count: geometry.vertices.len() as u32 * 3,
      vertex_buffer,
      index_buffer,
    }
  }
}
