mod mesh;
mod renderer;
mod viewport;

use genmesh::generators::{Cube, IcoSphere};
use gloo_console::log;
use gloo_timers::callback::Timeout;
use gloo_utils::{body, document, window};
use js_sys::Array;
use nalgebra::{Matrix4, Similarity};
use viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use mesh::{Geometry, Material, Mesh};
use renderer::Renderer;
use std::cell::RefCell;
use std::rc::Rc;

use rapier3d::prelude::*;

pub fn iter_to_array<T>(iterable: impl IntoIterator<Item = T>) -> Array
where
  T: Into<JsValue>,
{
  iterable.into_iter().map(|v| v.into()).collect::<Array>()
}

fn main() {
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      log!("Couldn't spawn async main", err);
    })
  })
}

async fn async_main() -> Result<(), JsValue> {
  let canvas = document().create_element("canvas")?;
  body().append_child(&canvas)?;
  let mut renderer = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;
  let viewport = Viewport::new(renderer.canvas());

  let geo = Geometry::from_genmesh(&Cube::new());
  let mat = Material {
    vertex_colors: geo.vertices.iter().map(|_| [1., 0., 0.]).collect(),
    texture_coordinates: vec![],
    texture_src: "/img/icon.png".to_string(),
  };
  let cube = Mesh::new(&renderer, &geo, &mat).await?;
  let geo = Geometry::from_genmesh(&IcoSphere::subdivide(3));
  let mat = Material {
    vertex_colors: geo.vertices.iter().map(|_| [0., 1., 0.]).collect(),
    texture_src: "/img/icon.png".to_string(),
    texture_coordinates: geo
      .vertices
      .iter()
      .map(|v| [(v[0] + 1.) / 2., 1. - (v[1] + 1.) / 2.])
      .collect(),
  };

  let sphere = Mesh::new(&renderer, &geo, &mat).await?;

  let body1 = RigidBodyBuilder::dynamic()
    .sleeping(false)
    .angvel(Vector::y())
    .build();
  let body2 = RigidBodyBuilder::fixed()
    .translation(vector![0., 0., 0.])
    .build();

  let mut rigid_body_set = RigidBodySet::new();
  let mut collider_set = ColliderSet::new();

  let handle1 = rigid_body_set.insert(body1);
  let handle2 = rigid_body_set.insert(body2);

  let gravity = vector![0.0, 0.0, 0.0];
  let integration_parameters = IntegrationParameters::default();
  let mut physics_pipeline = PhysicsPipeline::new();
  let mut island_manager = IslandManager::new();
  let mut broad_phase = BroadPhase::new();
  let mut narrow_phase = NarrowPhase::new();
  let mut impulse_joint_set = ImpulseJointSet::new();
  let mut multibody_joint_set = MultibodyJointSet::new();
  let mut ccd_solver = CCDSolver::new();
  let physics_hooks = ();
  let event_handler = ();

  let handles = vec![handle1, handle2];
  let scales = vec![1., 1.5];
  let meshes = vec![cube, sphere];

  on_animation_frame(
    move |_| {
      physics_pipeline.step(
        &gravity,
        &integration_parameters,
        &mut island_manager,
        &mut broad_phase,
        &mut narrow_phase,
        &mut rigid_body_set,
        &mut collider_set,
        &mut impulse_joint_set,
        &mut multibody_joint_set,
        &mut ccd_solver,
        None,
        &physics_hooks,
        &event_handler,
      );
      let bodies: Vec<Matrix4<f32>> = handles
        .iter()
        .zip(scales.iter())
        .map(|(handle, scale)| {
          let body = rigid_body_set.get(*handle).unwrap();
          Similarity::from_isometry(*body.position(), *scale).to_homogeneous()
        })
        .collect();
      renderer.render(&meshes, &bodies, viewport.view_proj());
    },
    None,
  );
  Ok(())
}

fn now() -> f64 {
  window().performance().unwrap().now()
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
  window()
    .request_animation_frame(f.as_ref().unchecked_ref())
    .expect("should register `requestAnimationFrame` OK")
}

pub fn on_animation_frame(mut closure: impl FnMut(f64) + 'static, fps: Option<f64>) {
  let t = Rc::new(RefCell::new(0.));
  let f = Rc::new(RefCell::new(None));
  let g = f.clone();
  let then = t.clone();
  let closure = Closure::wrap(Box::new(move || {
    let mut then = then.borrow_mut();
    let delta = now() - *then;
    closure(delta);
    *then = now();
    let h = f.clone();
    let next_frame = move || {
      request_animation_frame(h.borrow().as_ref().unwrap());
    };
    if let Some(fps) = fps {
      Timeout::new(((1000. / fps) - delta) as u32, next_frame).forget();
    } else {
      next_frame();
    };
  }) as Box<dyn FnMut()>);
  *g.borrow_mut() = Some(closure);
  *t.borrow_mut() = now();
  request_animation_frame(g.borrow().as_ref().unwrap());
}
