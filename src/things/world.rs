//! The environment: planet, skybox and global lighting. See [`World`].

use genmesh::generators::IcoSphere;
use nalgebra::{vector, Point3};
use noise::{Fbm, NoiseFn, Perlin};
use rapier3d::{dynamics::RigidBodyBuilder, geometry::ColliderBuilder};
use wasm_bindgen::JsValue;

use super::Backdrop;
use crate::core::{Color, Geometry, Group, Material, Mesh, Renderer};
use crate::lights::{Ambient, Sun};

/// Builder for the environment: a noise-displaced ocean planet inside a
/// skybox, lit by a sun and ambient light.
pub struct World;

impl World {
  /// Planet radius in world units (metres). Also caps camera zoom-out.
  pub const RADIUS: f32 = 1000.;

  /// Returns a `"world"` group containing:
  /// - `"skybox"`: see [`Backdrop`]. Also the environment PBR surfaces reflect.
  /// - `"sun"`: a warm [`Sun`] from above-front.
  /// - `"ambient"`: a faint bluish [`Ambient`] fill.
  /// - `"lithosphere"`: a fixed body centred `RADIUS + 10` below the origin,
  ///   with a displaced sphere mesh and a convex-hull collider.
  ///
  /// # Errors
  /// If mesh creation or a skybox image load fails.
  pub async fn new(renderer: &Renderer) -> Result<Group, JsValue> {
    // {
    //   let geo = Geometry::from_genmesh(&IcoSphere::subdivide(4));
    //   let mesh = Mesh::new(&renderer, &geo, &Material::new(Color::rgb(0., 0.2, 0.5))).await?;
    //
    //   let body = RigidBodyBuilder::fixed()
    //     .translation(vector![0., -1000., 0.])
    //     .build();
    //   Group::new("hydrosphere").with_mesh(mesh).with_body(body).with_scale(1000.)
    // }
    let lithosphere = {
      // Mesh resolution of the sphere.
      const SUBDIVISIONS: usize = 6;
      // Noise sampling frequency: higher = smaller, denser features.
      const FREQUENCY: f64 = 8.;
      // Displacement height relative to the sphere radius.
      const AMPLITUDE: f64 = 0.02;
      // Ocean albedo (linear), from troughs to crests.
      const DEEP: [f32; 3] = [0.0, 0.01, 0.05];
      const SHALLOW: [f32; 3] = [0.01, 0.17, 0.5];

      let mut geo = Geometry::from_genmesh(&IcoSphere::subdivide(SUBDIVISIONS));
      let noise = Fbm::<Perlin>::new(0);

      for v in geo.vertices.iter_mut() {
        let noise = noise.get([
          v[0] as f64 * FREQUENCY,
          v[1] as f64 * FREQUENCY,
          v[2] as f64 * FREQUENCY,
        ]);
        let d = 1. + AMPLITUDE * noise;
        v[0] *= d as f32;
        v[1] *= d as f32;
        v[2] *= d as f32;
      }
      // Displacement bends the surface, so re-derive normals for lighting.
      geo.compute_normals();
      let mesh = Mesh::new(
        renderer,
        &geo,
        &Material::vertex_color(
          geo
            .vertices
            .iter()
            .map(|v| {
              let pos = vector![v[0], v[1], v[2]];
              // Map displacement from [-1, 1] to [0, 1].
              let d = (pos.magnitude() - 1.0) / AMPLITUDE as f32;
              let t = (0.5 + 0.5 * d).clamp(0., 1.);
              [
                DEEP[0] + (SHALLOW[0] - DEEP[0]) * t,
                DEEP[1] + (SHALLOW[1] - DEEP[1]) * t,
                DEEP[2] + (SHALLOW[2] - DEEP[2]) * t,
              ]
            })
            .collect(),
        )
        // Water: glossy dielectric, reflects the sky at grazing angles.
        .with_roughness(0.25),
      )
      .await?;
      let vertices = geo
        .vertices
        .iter()
        .map(|[x, y, z]| Point3::new(x * Self::RADIUS, y * Self::RADIUS, z * Self::RADIUS))
        .collect();
      let indices: Vec<[u32; 3]> = geo.indices.chunks(3).map(|v| [v[0], v[1], v[2]]).collect();
      let lithocollider = ColliderBuilder::convex_mesh(vertices, &indices)
        .unwrap()
        .build();

      let body = RigidBodyBuilder::fixed()
        .translation(vector![0., -Self::RADIUS - 10., 0.])
        .build();
      Group::new("lithosphere")
        .with_mesh(mesh)
        .with_body(body)
        .with_collider(lithocollider)
        .with_scale(Self::RADIUS)
    };
    let sun = Group::new("sun").with_light(Sun::new(
      vector![-0.4, -1., -0.6],
      Color::rgb(1., 0.95, 0.85),
      3.,
    ));
    let ambient = Group::new("ambient").with_light(Ambient::new(Color::rgb(0.6, 0.7, 1.), 0.05));
    Ok(
      Group::new("world")
        .with_child(Backdrop::new(renderer).await?)
        .with_child(sun)
        .with_child(ambient)
        .with_child(lithosphere),
    )
  }
}
