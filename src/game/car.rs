//! Car model loaded from glTF (currently unused). See [`Car`].

use nalgebra::{Similarity3, Translation3, UnitQuaternion, Vector3};
use wasm_bindgen::JsValue;

use crate::core::{Geometry, Group, Mesh, Renderer};

/// Builder for the car visuals from `gltf/CarConcept.glb`.
///
/// Not currently added to the scene. To attach it to the player body:
/// ```ignore
/// scene.add_group_to("car", Car::new(&renderer).await?)?;
/// ```
pub struct Car;

impl Car {
  /// Id of the root visuals node.
  pub const NODE: &'static str = "car_visuals";

  /// Returns a [`Car::NODE`] group, rotated 180° about Y to face forward,
  /// with one `car_{i}` child per glTF primitive.
  ///
  /// # Errors
  /// If the file fails to fetch/parse or a mesh fails to upload.
  pub async fn new(renderer: &Renderer) -> Result<Group, JsValue> {
    let glb_bytes = Renderer::fetch_bytes("gltf/CarConcept.glb").await?;
    let gltf_nodes =
      Geometry::from_gltf(&glb_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut group = Group::new(Self::NODE).with_transform(Similarity3::from_parts(
      Translation3::identity(),
      UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f32::consts::PI),
      1.,
    ));

    for (i, (geo, material, local_sim)) in gltf_nodes.into_iter().enumerate() {
      let mesh = Mesh::new(renderer, &geo, &material).await?;
      group.add_child(
        Group::new(format!("car_{i}"))
          .with_mesh(mesh)
          .with_transform(local_sim),
      );
    }

    Ok(group)
  }
}
