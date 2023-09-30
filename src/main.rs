mod mesh;
mod renderer;
mod scene;
mod viewport;

use genmesh::{Triangulate, Vertices};
use nalgebra::{Point3, Vector};
use noise::{Curve, Fbm, NoiseFn, Perlin};

use renderer::Color;

use fluid::{add_event_and_forget, on_animation_frame, Context};
use fluid_macro::html;
use genmesh::generators::{Cube, IcoSphere, IndexedPolygon, SharedVertex};
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
      .translation(vector![4., 4., 0.])
      .build();

    scene.add("cube", mesh, body);

    let geo = Geometry::from_genmesh(&IcoSphere::subdivide(3));
    let mesh = Mesh::new(
      &renderer,
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

    let body = RigidBodyBuilder::fixed().build();
    scene.add_w_scale("skybox", mesh, body, 10000.);

    let mesh = Mesh::new(
      &renderer,
      &Geometry::from_genmesh(&IcoSphere::subdivide(3)),
      &Material::new(Color::rgb(1., 0., 0.)),
    )
    .await?;

    let body = RigidBodyBuilder::dynamic()
      .sleeping(true)
      .translation(vector![0., 2., 0.])
      .additional_mass(2.)
      .build();

    scene.add("sphere", mesh, body);
  }
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
      &renderer,
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
    let geo = Geometry::from_genmesh(&IcoSphere::subdivide(3));
    let mesh = Mesh::new(
      &renderer,
      &geo,
      &Material::vertex_color(geo.vertices.clone()),
    )
    .await?;

    let body = RigidBodyBuilder::dynamic()
      .sleeping(false)
      .translation(vector![-4., 1., 0.])
      .additional_mass(2.)
      .build();
    let ball = ColliderBuilder::ball(1.).build();

    scene.add_w_scale_collider("vertex_cube", mesh, body, ball, 1.);
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

  let mut first_frame = true;

  on_animation_frame(
    move |_| {
      if !*paused.get() || first_frame {
        scene.physics();
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
          &viewport.borrow(),
        );
      }
      if first_frame {
        first_frame = false;
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
