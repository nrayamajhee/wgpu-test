use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fluid::add_event_and_forget;
use gloo_utils::window as gloo_window;
use nalgebra::{vector, Similarity3, UnitQuaternion, Vector3};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::KeyboardEvent;

use crate::{
  scene::{NodeBundle, SceneNode},
  Geometry, Mesh, Renderer, Scene, Viewport,
};

#[derive(Clone, Copy, PartialEq)]
pub enum MovementSpeed {
  Normal,
  Fast,
}

pub struct Player {
  pub speed: Rc<RefCell<MovementSpeed>>,
  scene: Rc<RefCell<Scene>>,
  viewport: Rc<RefCell<Viewport>>,
}

impl Player {
  pub async fn new(
    _renderer: &Renderer,
    scene: Rc<RefCell<Scene>>,
    viewport: Rc<RefCell<Viewport>>,
  ) -> Result<Self, JsValue> {
    let speed = Rc::new(RefCell::new(MovementSpeed::Normal));

    {
      let speed = speed.clone();
      add_event_and_forget(&gloo_window(), "keydown", move |e| {
        if e.dyn_into::<KeyboardEvent>().unwrap().key() == "Shift" {
          *speed.borrow_mut() = MovementSpeed::Fast;
        }
      });
    }
    {
      let speed = speed.clone();
      add_event_and_forget(&gloo_window(), "keyup", move |e| {
        if e.dyn_into::<KeyboardEvent>().unwrap().key() == "Shift" {
          *speed.borrow_mut() = MovementSpeed::Normal;
        }
      });
    }

    Ok(Self {
      speed,
      scene,
      viewport,
    })
  }

  pub async fn node(renderer: &Renderer) -> Result<NodeBundle, JsValue> {
    let glb_bytes = Renderer::fetch_bytes("gltf/CarConcept.glb").await?;
    let gltf_nodes =
      Geometry::from_gltf(&glb_bytes).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let mut nodes = vec![SceneNode {
      id: "car_visuals".to_owned(),
      parent: None, // Will be set to "car" by Scene::insert_bundle if we handle it correctly
      children: (0..gltf_nodes.len()).map(|i| format!("car_{i}")).collect(),
    }];
    let mut transforms = HashMap::new();
    let mut meshes = HashMap::new();

    transforms.insert(
      "car_visuals".to_owned(),
      Similarity3::from_parts(
        nalgebra::Translation3::identity(),
        nalgebra::UnitQuaternion::from_axis_angle(
          &nalgebra::Vector3::y_axis(),
          std::f32::consts::PI,
        ),
        1.,
      ),
    );

    for (i, (geo, material, local_sim)) in gltf_nodes.into_iter().enumerate() {
      let id = format!("car_{i}");
      let mesh = Mesh::new(renderer, &geo, &material).await?;
      nodes.push(SceneNode {
        id: id.clone(),
        parent: Some("car_visuals".to_owned()),
        children: vec![],
      });
      transforms.insert(id.clone(), local_sim);
      meshes.insert(id, mesh);
    }

    // Explicitly set "car_visuals" parent to "car"
    nodes[0].parent = Some("car".to_owned());

    Ok(NodeBundle {
      nodes,
      transforms,
      meshes,
    })
  }

  pub fn face_viewport(&self) {
    let viewport = self.viewport.borrow();
    let facing = viewport.facing_xz();
    let forward = Vector3::new(facing.x, 0., facing.y);
    let target_rot = UnitQuaternion::face_towards(&-forward, &Vector3::y());
    let mut scene = self.scene.borrow_mut();
    let body = scene.get_body_mut("car").unwrap();
    body.set_rotation(target_rot, true);
  }

  pub fn move_(&self, dx: isize, dy: isize) {
    let viewport = self.viewport.borrow();
    let facing = viewport.facing_xz();
    let forward = Vector3::new(facing.x, 0., facing.y);
    let right = Vector3::new(-facing.y, 0., facing.x);
    let magnitude = match *self.speed.borrow() {
      MovementSpeed::Normal => 10.,
      MovementSpeed::Fast => 30.,
    };
    let impulse = forward * (dy as f32 * magnitude) + right * (dx as f32 * magnitude);
    let mut scene = self.scene.borrow_mut();
    let body = scene.get_body_mut("car").unwrap();
    body.apply_impulse(vector![impulse.x, 0., impulse.z], true);
  }
}
