//! CPU-side triangle meshes. See [`Geometry`].

use genmesh::{
  generators::{IndexedPolygon, SharedVertex},
  EmitTriangles, Triangulate, Vertex,
};
use nalgebra::Vector3;

/// Indexed triangle list on the CPU.
///
/// `normals` and `tangents` are per vertex. Either may be empty; the mesh
/// upload then fills in computed normals or zero tangents (zero tangents
/// disable normal mapping for that mesh).
pub struct Geometry {
  /// Vertex positions.
  pub vertices: Vec<[f32; 3]>,
  /// Unit vertex normals.
  pub normals: Vec<[f32; 3]>,
  /// Unit tangents along +U, `w` = bitangent sign (glTF convention).
  pub tangents: Vec<[f32; 4]>,
  /// Triangle indices into `vertices`, three per triangle, CCW front faces.
  pub indices: Vec<u32>,
}

impl Geometry {
  /// Triangulates any [`genmesh`] generator (e.g. `IcoSphere`, `Cube`),
  /// keeping its normals.
  pub fn from_genmesh<T, P>(primitive: &T) -> Self
  where
    P: EmitTriangles<Vertex = usize>,
    T: SharedVertex<Vertex> + IndexedPolygon<P>,
  {
    let (vertices, normals) = primitive
      .shared_vertex_iter()
      .map(|v| -> ([f32; 3], [f32; 3]) { (v.pos.into(), v.normal.into()) })
      .unzip();
    let indices: Vec<u32> = primitive
      .indexed_polygon_iter()
      .triangulate()
      .flat_map(|i| [i.x as u32, i.y as u32, i.z as u32])
      .collect();
    Geometry {
      vertices,
      normals,
      tangents: vec![],
      indices,
    }
  }

  /// Box centred on the origin with `[width, height, depth]` along X, Y, Z,
  /// with flat per-face normals.
  ///
  /// Use this for non-uniform shapes: [`Group`](crate::core::Group)
  /// transforms only support uniform scale.
  pub fn cuboid([width, height, depth]: [f32; 3]) -> Self {
    let mut geometry = Self::from_genmesh(&genmesh::generators::Cube::new());
    for v in geometry.vertices.iter_mut() {
      // Cube spans -1..1, so scale by half-extents. Face normals stay
      // axis-aligned, so they remain valid.
      v[0] *= width / 2.;
      v[1] *= height / 2.;
      v[2] *= depth / 2.;
    }
    geometry
  }

  /// Square in the XZ plane, facing +Y, spanning `-half_size..half_size`.
  pub fn plane(half_size: f32) -> Self {
    let s = half_size;
    Geometry {
      vertices: vec![[-s, 0., -s], [s, 0., -s], [s, 0., s], [-s, 0., s]],
      normals: vec![[0., 1., 0.]; 4],
      tangents: vec![],
      indices: vec![0, 2, 1, 0, 3, 2],
    }
  }

  /// Iterates triangles as index triples.
  fn triangles(&self) -> impl Iterator<Item = [usize; 3]> + '_ {
    self
      .indices
      .chunks_exact(3)
      .map(|t| [t[0] as usize, t[1] as usize, t[2] as usize])
  }

  /// Recomputes smooth normals by averaging face normals, weighted by area.
  /// Call after moving vertices (e.g. noise displacement).
  pub fn compute_normals(&mut self) {
    let mut sums = vec![Vector3::<f32>::zeros(); self.vertices.len()];
    for [a, b, c] in self.triangles() {
      let [pa, pb, pc] = [a, b, c].map(|i| Vector3::from(self.vertices[i]));
      // Unnormalized cross product: length is twice the triangle area.
      let face = (pb - pa).cross(&(pc - pa));
      for i in [a, b, c] {
        sums[i] += face;
      }
    }
    self.normals = sums
      .into_iter()
      .map(|n| n.try_normalize(1e-12).unwrap_or(Vector3::y()).into())
      .collect();
  }

  /// Computes per-vertex tangents from `uvs` (one per vertex), for normal
  /// mapping. Requires `normals`. Accumulates per-triangle tangent frames,
  /// then orthonormalizes against the normal (a simplified MikkTSpace).
  pub fn compute_tangents(&mut self, uvs: &[[f32; 2]]) {
    let count = self.vertices.len();
    if uvs.len() != count || self.normals.len() != count {
      return;
    }
    let mut tangents = vec![Vector3::<f32>::zeros(); count];
    let mut bitangents = vec![Vector3::<f32>::zeros(); count];
    for [a, b, c] in self.triangles() {
      let [pa, pb, pc] = [a, b, c].map(|i| Vector3::from(self.vertices[i]));
      let [ua, ub, uc] = [a, b, c].map(|i| uvs[i]);
      let (e1, e2) = (pb - pa, pc - pa);
      let (du1, dv1) = (ub[0] - ua[0], ub[1] - ua[1]);
      let (du2, dv2) = (uc[0] - ua[0], uc[1] - ua[1]);
      let det = du1 * dv2 - du2 * dv1;
      if det.abs() < 1e-12 {
        continue;
      }
      let tangent = (e1 * dv2 - e2 * dv1) / det;
      let bitangent = (e2 * du1 - e1 * du2) / det;
      for i in [a, b, c] {
        tangents[i] += tangent;
        bitangents[i] += bitangent;
      }
    }
    self.tangents = (0..count)
      .map(|i| {
        let n = Vector3::from(self.normals[i]);
        let t = tangents[i] - n * n.dot(&tangents[i]);
        match t.try_normalize(1e-12) {
          Some(t) => {
            let w = if n.cross(&t).dot(&bitangents[i]) < 0. {
              -1.
            } else {
              1.
            };
            [t.x, t.y, t.z, w]
          }
          None => [0.; 4],
        }
      })
      .collect();
  }
}
