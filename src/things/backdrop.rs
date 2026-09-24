//! Skybox. See [`Backdrop`].

use genmesh::generators::IcoSphere;
use wasm_bindgen::JsValue;

use crate::core::{Geometry, Group, Material, Mesh, Renderer};

/// Builder for the Milky Way skybox.
pub struct Backdrop;

impl Backdrop {
  /// Returns a `"skybox"` group: a large icosphere with a cube map from
  /// `img/milkyway/`. Rendered with camera rotation only, so it appears infinitely far.
  ///
  /// # Errors
  /// If any face image fails to load.
  pub async fn new(renderer: &Renderer) -> Result<Group, JsValue> {
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

    Ok(Group::new("skybox").with_mesh(mesh).with_scale(10000.))
  }
}
