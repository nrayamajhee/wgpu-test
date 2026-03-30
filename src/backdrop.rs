use genmesh::generators::IcoSphere;
use nalgebra::Similarity3;
use wasm_bindgen::JsValue;

use crate::{Geometry, Material, Mesh, Renderer, Scene};

pub struct Backdrop {}

impl Backdrop {
  pub async fn new(renderer: &Renderer, scene: &mut Scene) -> Result<Self, JsValue> {
    let geo = Geometry::from_genmesh(&IcoSphere::subdivide(3));
    let mesh = Mesh::new(
      renderer,
      &geo,
      &Material::cubemap([
        "img/milkyway/posx.jpg",
        "img/milkyway/negx.jpg",
        "img/milkyway/posy.jpg",
        "img/milkyway/negy.jpg",
        "img/milkyway/posz.jpg",
        "img/milkyway/negz.jpg",
      ]),
    )
    .await?;

    scene.add_static("skybox", mesh, Similarity3::from_scaling(10000.));

    Ok(Self {})
  }
}
