mod game;
mod mesh;
mod movement;
mod renderer;
mod scene;
mod viewport;

pub use game::Game;
pub use mesh::{Geometry, Material, Mesh};
use movement::Movement;
use renderer::Color;
pub use renderer::Renderer;
pub use scene::Scene;
pub use viewport::Viewport;

use genmesh::{Triangulate, Vertices};
use nalgebra::{Point3, Vector};
use noise::{Curve, Fbm, NoiseFn, Perlin};

use fluid::{add_event_and_forget, on_animation_frame, Context};
use fluid_macro::html;
use genmesh::generators::{Cube, IcoSphere, IndexedPolygon, SharedVertex};
use gloo_console::log;
use gloo_utils::{body, document, window as gloo_window};
use js_sys::Array;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent};

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
  let renderer = Renderer::new().await?;
  let viewport = Viewport::new(renderer.canvas());
  let context = Context::new();
  let viewport = Rc::new(RefCell::new(viewport));
  let mut scene = Scene::new();

  body().append_child(&renderer.canvas())?;

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

  let renderer = Rc::new(RefCell::new(renderer));
  let game = Rc::new(Game::new(&context, renderer.clone(), viewport.clone()));
  let game = Rc::new(game);
  {
    let renderer = renderer.clone();
    let viewport = viewport.clone();
    let w1 = game.clone();
    let w2 = w1.clone();
    let w3 = w1.clone();
    let w4 = w1.clone();
    let ui = html! {
        div class=[context, &format!("overlay {}",if w1.paused() {"shown"} else {""})] {
            div class="pause-menu" {
             h1 {"Pause Menu" }
             div class="buttons" {
                 button
                 class="resume-btn"
                 @click=(move |_| {
                     w2.resume(renderer.clone(), &mut viewport.borrow_mut())
                 })
                 { "Resume" }
                 button
                 @click=(move |_| {
                     w3.toggle_fullscreen();
                 })
                 {[
                     context,
                     if w4.fullscreen() {
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

  let movement = Rc::new(RefCell::new(Movement { dx: 0., dy: 0. }));

  {
    let viewport = viewport.clone();

    fluid::add_event_and_forget(&gloo_window(), "wheel", move |e| {
      viewport
        .borrow_mut()
        .update_zoom(e.dyn_into::<WheelEvent>().unwrap().delta_y() as i32);
    });
  }
  {
    let viewport = viewport.clone();

    add_event_and_forget(&gloo_window(), "mousemove", move |e| {
      let me = e.dyn_into::<MouseEvent>().unwrap();
      viewport
        .borrow_mut()
        .update_rot(me.movement_x(), me.movement_y(), 1.);
    });
  }
  {
    let movement = movement.clone();
    add_event_and_forget(&gloo_window(), "keydown", move |e| {
      let key = e.dyn_into::<KeyboardEvent>().unwrap().key();
      match key.as_str() {
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
    add_event_and_forget(&gloo_window(), "keyup", move |e| {
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
      if !game.paused() || first_frame {
        scene.physics();
        let Movement { dx, dy } = *movement.borrow();
        let body = scene.get_body_mut("sphere").unwrap();
        if dx == 0. && dy == 0. {
          body.reset_forces(true);
        } else {
          body.apply_impulse(vector![dx * 0.1, 0., -dy * 0.1], true);
        }
        viewport.borrow_mut().follow(*body.position());
        renderer
          .borrow_mut()
          .render(&scene.meshes(), &scene.simiarities(), &viewport.borrow());
      }
      if first_frame {
        first_frame = false;
      }
    },
    None,
  );
  Ok(())
}
