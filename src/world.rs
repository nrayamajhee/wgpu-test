use genmesh::generators::IcoSphere;
use nalgebra::{vector, Point3};
use noise::{Fbm, NoiseFn, Perlin};
use rapier3d::{dynamics::RigidBodyBuilder, geometry::ColliderBuilder};
use wasm_bindgen::JsValue;

use crate::{Geometry, Material, Mesh, Renderer};

pub struct World {}

impl World {
  pub async fn new(renderer: &Renderer, scene: &mut crate::Scene) -> Result<Self, JsValue> {
    // {
    //   let geo = Geometry::from_genmesh(&IcoSphere::subdivide(4));
    //   let mesh = Mesh::new(&renderer, &geo, &Material::new(Color::rgb(0., 0.2, 0.5))).await?;
    //
    //   let body = RigidBodyBuilder::fixed()
    //     .translation(vector![0., -1000., 0.])
    //     .build();
    //   scene.add_w_scale("hydrosphere", mesh, body, 1000.);
    // }
    {
      let mut geo = Geometry::from_genmesh(&IcoSphere::subdivide(4));
      let noise = Fbm::<Perlin>::new(0);

      for v in geo.vertices.iter_mut() {
        let noise = noise.get([v[0] as f64, v[1] as f64, v[2] as f64]);
        let d = 1. + 0.1 * noise;
        v[0] *= d as f32;
        v[1] *= d as f32;
        v[2] *= d as f32;
      }
      let mesh = Mesh::new(
        renderer,
        &geo,
        &Material::vertex_color(
          geo
            .vertices
            .iter()
            .map(|v| {
              let pos = vector![v[0], v[1], v[2]];
              let d = (pos.magnitude() - 1.0) / 0.1;
              [0., 0.2 + 0.2 * d, 0.]
            })
            .collect(),
        ),
      )
      .await?;
      let vertices = geo
        .vertices
        .iter()
        .map(|[x, y, z]| Point3::new(x * 1000., y * 1000., z * 1000.))
        .collect();
      let indices: Vec<[u32; 3]> = geo
        .indices
        .chunks(3)
        .map(|v| [v[0] as u32, v[1] as u32, v[2] as u32])
        .collect();
      let lithocollider = ColliderBuilder::convex_mesh(vertices, &indices)
        .unwrap()
        .build();

      let body = RigidBodyBuilder::fixed()
        .translation(vector![0., -1010., 0.])
        .build();
      scene.add_w_scale_collider("lithosphere", mesh, body, lithocollider, 1000.);
    }
    Ok(Self {})
  }
}
