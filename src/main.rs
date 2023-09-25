mod mesh;
mod renderer;
mod scene;
mod viewport;

use renderer::Color;

use fluid::{add_event_and_forget, on_animation_frame, Context};
use fluid_macro::html;
use genmesh::generators::{Cube, IcoSphere};
use gloo_console::log;
use gloo_utils::{body, document, window};
use js_sys::Array;
use scene::Scene;
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
  let canvas = document().create_element("canvas")?;
  body().append_child(&canvas)?;

  let ctx = Context::new();
  let paused = ctx.create_signal(true);
  let fullscreen = ctx.create_signal(false);
  let pointer_grabbed = ctx.create_signal(false);
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

  let mut scene = Scene::new();
  let renderer = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;

  {
    let geo = Geometry::from_genmesh(&Cube::new());
    let mesh = Mesh::new(
      &renderer,
      &geo,
      &Material::textured(
        "img/icon.png",
        geo
          .vertices
          .iter()
          .map(|v| [(v[0] + 1.) / 2., 1. - (v[1] + 1.) / 2.])
          .collect(),
      ),
    )
    .await?;

    let body = RigidBodyBuilder::dynamic()
      .sleeping(false)
      .angvel(Vector::y())
      .translation(vector![4., 0., 0.])
      .build();

    scene.add("cube", mesh, body);

    let geo = Geometry::from_genmesh(&Cube::new());
    let mesh = Mesh::new(
      &renderer,
      &geo,
      &Material::cubemap(
        [
          "img/milkyway/posx.jpg",
          "img/milkyway/negx.jpg",
          "img/milkyway/posy.jpg",
          "img/milkyway/negy.jpg",
          "img/milkyway/posz.jpg",
          "img/milkyway/negz.jpg",
        ]
      ),
    )
    .await?;

    let body = RigidBodyBuilder::fixed()
      .build();
    scene.add_w_scale("skybox", mesh, body, 1000.);

    let mesh = Mesh::new(
      &renderer,
      &Geometry::from_genmesh(&IcoSphere::subdivide(3)),
      &Material::new(Color::rgb(1., 0., 0.)),
    )
    .await?;

    let body = RigidBodyBuilder::dynamic()
      .sleeping(true)
      .additional_mass(2.)
      .build();

    scene.add("sphere", mesh, body);
  }

  let viewport = Rc::new(RefCell::new(Viewport::new(renderer.canvas())));
  let movement = Rc::new(RefCell::new(Movement { dx: 0., dy: 0. }));
  let renderer = Rc::new(RefCell::new(renderer));

  {
    add_event_and_forget(&document(), "fullscreenchange", move |_| {
      let f = *fullscreen.get();
      fullscreen.set(!f);
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

  renderer.borrow_mut().render(
    &scene.meshes(),
    &scene.simiarities(),
    viewport.borrow().view_proj(),
  );

  on_animation_frame(
    move |_| {
      scene.physics();
      if !*paused.get() {
        let Movement { dx, dy } = *movement.borrow();
        let body = scene.get_body_mut("sphere").unwrap();
        if dx == 0. && dy == 0. {
          body.reset_forces(true);
        } else {
          body.apply_impulse(vector![dx * 0.1, 0., -dy * 0.1], true);
        }
        viewport.borrow_mut().follow(*body.position());
        renderer.borrow_mut().render(
          &scene.meshes(),
          &scene.simiarities(),
          viewport.borrow().view_proj(),
        );
      }
    },
    None,
  );
  Ok(())
}

#[derive(Debug)]
struct Movement {
  dx: f32,
  dy: f32,
}
