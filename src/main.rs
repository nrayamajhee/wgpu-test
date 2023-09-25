mod mesh;
mod renderer;
mod viewport;

use renderer::Color;

use fluid::{add_event_and_forget, Context};
use fluid_macro::html;
use genmesh::generators::{Cube, IcoSphere};
use gloo_console::log;
use gloo_timers::callback::Timeout;
use gloo_utils::{body, document, window};
use js_sys::Array;
use nalgebra::{Matrix4, Similarity};
use viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent};

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
  let ctx = Context::new();
  let paused = ctx.create_signal(true);
  let fullscreen = ctx.create_signal(false);
  let pointer_grabbed = ctx.create_signal(false);
  ctx.create_effect(move || {});
  let canvas = document().create_element("canvas")?;
  body().append_child(&canvas)?;
  {
    let p1 = paused.clone();
    let f1 = fullscreen.clone();
    let f2 = fullscreen.clone();
    let c1 = canvas.clone();
    let ui = html! {
        div class=[ctx, &format!("overlay {}",if *p1.get() {"shown"} else {""})] {
            div class="pause-menu" {
             h1 {"Pause Menu" }
             div class="buttons" {
                 button
                 class="resume-btn"
                 @click=(move |_| {
                     c1.request_pointer_lock();
                 })
                 { "Resume" }
                 button
                 @click=(move |_| {
                     if *f1.get() {
                         document().exit_fullscreen();
                     } else {
                         document().document_element().unwrap().request_fullscreen().unwrap();
                     }
                 })
                 {[
                     ctx,
                     if *f2.get() {
                         "Exit fullscreen"
                     } else {
                         "Go fullscreeen"
                     }
                 ]}
             }
            }
        }
    };
    body().append_child(&ui)?;
  }
  {
    add_event_and_forget(&document(), "fullscreenchange", move |_| {
      let f = *fullscreen.get();
      fullscreen.set(!f);
    });
  }
  let movement = Rc::new(RefCell::new(Movement { dx: 0., dy: 0. }));
  {
    let paused = paused.clone();
    let movement = movement.clone();
    add_event_and_forget(&window(), "keydown", move |e| {
      let key = e.dyn_into::<KeyboardEvent>().unwrap().key();
      match key.as_str() {
        "Escape" => {
          let p = *paused.get();
          if !p {
            document().exit_pointer_lock();
          }
        }
        "w" => {
          movement.borrow_mut().dy += 1.;
        }
        "s" => {
          movement.borrow_mut().dy -= 1.;
        }
        "a" => {
          movement.borrow_mut().dx -= 1.;
        }
        "d" => {
          movement.borrow_mut().dx += 1.;
        }
        _ => {}
      }
    });
  }
  {
    let movement = movement.clone();
    add_event_and_forget(&window(), "keyup", move |e| {
      let key = e.dyn_into::<KeyboardEvent>().unwrap().key();
      match key.as_str() {
        "w" => {
          movement.borrow_mut().dy -= 1.;
        }
        "s" => {
          movement.borrow_mut().dy += 1.;
        }
        "a" => {
          movement.borrow_mut().dx += 1.;
        }
        "d" => {
          movement.borrow_mut().dx -= 1.;
        }
        _ => {}
      }
    });
  }
  let mut renderer = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;
  let viewport = Viewport::new(renderer.canvas());

  let geo = Geometry::from_genmesh(&Cube::new());
  let mat = Material::textured(
    "img/icon.png",
    geo
      .vertices
      .iter()
      .map(|v| [(v[0] + 1.) / 2., 1. - (v[1] + 1.) / 2.])
      .collect(),
  );
  let cube = Mesh::new(&renderer, &geo, &mat).await?;
  let geo = Geometry::from_genmesh(&IcoSphere::subdivide(3));
  let mat = Material::new(Color {
    r: 1.,
    g: 0.,
    b: 0.,
    a: 1.,
  });

  let sphere = Mesh::new(&renderer, &geo, &mat).await?;

  let body1 = RigidBodyBuilder::dynamic()
    .sleeping(false)
    .angvel(Vector::y())
    .translation(vector![4., 0., 0.])
    .build();
  let body2 = RigidBodyBuilder::dynamic()
    .sleeping(true)
    .additional_mass(2.)
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

  let meshes = vec![cube, sphere];
  let handles = vec![handle1, handle2];
  let scales = vec![1., 1.];

  let bodies: Vec<Matrix4<f32>> = handles
    .iter()
    .zip(scales.iter())
    .map(|(handle, scale)| {
      let body = rigid_body_set.get(*handle).unwrap();
      Similarity::from_isometry(*body.position(), *scale).to_homogeneous()
    })
    .collect();
  renderer.render(&meshes, &bodies, viewport.view_proj());

  let renderer = Rc::new(RefCell::new(renderer));
  let viewport = Rc::new(RefCell::new(viewport));

  {
    let renderer = renderer.clone();
    let viewport = viewport.clone();
    fluid::add_event_and_forget(&window(), "resize", move |_| {
      renderer.borrow_mut().resize();
      viewport.borrow_mut().resize(&renderer.borrow().canvas());
    });
  }

  {
    let viewport = viewport.clone();

    fluid::add_event_and_forget(&window(), "wheel", move |e| {
      viewport
        .borrow_mut()
        .update_zoom(e.dyn_into::<WheelEvent>().unwrap().delta_y() as i32);
    });
  }
  {
    let viewport = viewport.clone();

    add_event_and_forget(&window(), "mousemove", move |e| {
      let me = e.dyn_into::<MouseEvent>().unwrap();
      viewport
        .borrow_mut()
        .update_rot(me.movement_x(), me.movement_y(), 1.);
    });
  }
  {
    let pointer_grabbed = pointer_grabbed.clone();
    let paused = paused.clone();
    let viewport = viewport.clone();

    add_event_and_forget(&document(), "pointerlockchange", move |_| {
      let p = *paused.get();
      paused.set(!p);
      if p {
        viewport.borrow_mut().unlock();
      } else {
        viewport.borrow_mut().lock();
      }
      let p = *pointer_grabbed.get();
      pointer_grabbed.set(!p);
    });
  }

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
      if !*paused.get() {
        let Movement { dx, dy } = *movement.borrow();
        let body = rigid_body_set.get_mut(handle2).unwrap();
        if dx == 0. && dy == 0. {
          // body.reset_forces(true);
        } else {
          body.add_force(vector![dx * 0.01, 0., -dy * 0.01], true);
        }
        viewport.borrow_mut().follow(*body.position());
        let bodies: Vec<Matrix4<f32>> = handles
          .iter()
          .zip(scales.iter())
          .map(|(handle, scale)| {
            let body = rigid_body_set.get(*handle).unwrap();
            Similarity::from_isometry(*body.position(), *scale).to_homogeneous()
          })
          .collect();
        renderer
          .borrow_mut()
          .render(&meshes, &bodies, viewport.borrow().view_proj());
      }
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

#[derive(Debug)]
struct Movement {
  dx: f32,
  dy: f32,
}
