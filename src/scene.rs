use crate::mesh::{Geometry, Primitive, Vertex};
use genmesh::{
  generators::{Circle, IndexedPolygon, Plane, SharedVertex},
  EmitTriangles, Triangulate, Vertex as V,
};
use log::info;
use web_sys::GpuDevice;

pub struct Scene<'a> {
  device: &'a GpuDevice,
}

impl<'a> Scene<'a> {
  pub fn new(device: &'a GpuDevice) -> Self {
    let s = Self { device };
    info!("Scene created!");
    s
  }
  fn generate<T, P>(primitive: &T) -> Geometry
  where
    P: EmitTriangles<Vertex = usize>,
    T: SharedVertex<V> + IndexedPolygon<P>,
  {
    let vertices: Vec<_> = primitive
      .shared_vertex_iter()
      .map(|v| Vertex {
        position: v.pos.into(),
        tex_coords: [(v.pos.x + 1.) as f32 / 2., 1. - (v.pos.y + 1.) / 2.],
      })
      .collect();
    let indices: Vec<u16> = primitive
      .indexed_polygon_iter()
      .triangulate()
      .flat_map(|i| [i.x as u16, i.y as u16, i.z as u16])
      .collect();
    Geometry { vertices, indices }
  }
  pub fn primitive(&self, primitive: Primitive) -> Geometry {
    match primitive {
      Primitive::Plane(subdivision) => {
        let (x, y) = subdivision.unwrap_or((1, 1));
        Self::generate(&Plane::subdivide(x, y))
      }
      Primitive::Circle(subdivision) => {
        let s = subdivision.unwrap_or(4);
        Self::generate(&Circle::new(s))
      }
    }
  }
}
