mod backdrop;
mod game;
mod mesh;
mod movement;
mod player;
mod renderer;
mod scene;
mod viewport;
mod world;

use backdrop::Backdrop;
pub use game::Game;
pub use mesh::{Geometry, Material, Mesh};
use movement::Movement;
use player::Player;
use renderer::Color;
pub use renderer::Renderer;
pub use scene::{NodeBundle, Scene, SceneNode};
pub use viewport::Viewport;
use world::World;

use nalgebra::{Similarity3, Translation3, UnitQuaternion};

use fluid::{on_animation_frame, Context};
use fluid_macro::html;
use gloo_console::log;
use gloo_utils::body;
use js_sys::Array;
use wasm_bindgen::prelude::*;

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
  let ctx = Context::new();
  let viewport = Rc::new(RefCell::new(viewport));
  let scene = Rc::new(RefCell::new(Scene::new()));

  body().append_child(renderer.canvas())?;

  Backdrop::new(&renderer, &mut scene.borrow_mut()).await?;

  {
    let car_body = RigidBodyBuilder::dynamic()
      .sleeping(false)
      .translation(vector![0., 0., 0.])
      .additional_mass(1.)
      .linear_damping(10.)
      .build();
    let car_collider = ColliderBuilder::ball(1.).build();
    scene
      .borrow_mut()
      .add_w_scale_collider("car", None, car_body, car_collider, 1.);
    let car_node = Player::node(&renderer).await?;
    scene.borrow_mut().insert_bundle(car_node);
  }

  let player = Player::new(&renderer, scene.clone(), viewport.clone()).await?;

  {
    let geo = Geometry::plane(10.);
    let mesh = Mesh::new(&renderer, &geo, &Material::new(Color::rgb(0.1, 0.1, 0.1))).await?;
    let transform = Similarity3::from_parts(
      Translation3::new(0., -1., 0.),
      UnitQuaternion::identity(),
      1.,
    );
    scene.borrow_mut().add_static("plane", mesh, transform);
  }

  World::new(&renderer, &mut scene.borrow_mut()).await?;

  let renderer = Rc::new(RefCell::new(renderer));
  let game = Rc::new(Game::new(&ctx, renderer.clone(), viewport.clone()));
  let game = Rc::new(game);
  {
    let renderer = renderer.clone();
    let viewport = viewport.clone();
    let w1 = game.clone();
    let w2 = game.clone();
    let ui = html! {
        div class=#{ |game| &format!("overlay {}", if game.paused() { "shown" } else { "" }) } {
            div class="pause-menu" {
             h1 { "Pause Menu" }
             div class="buttons" {
                 button
                 class="resume-btn"
                 @click={ move |_| {
                     w1.resume(renderer.clone(), &mut viewport.borrow_mut())
                 } }
                 { "Resume" }
                 button
                 @click={ move |_| {
                     w2.toggle_fullscreen();
                 } }
                 {
                     #{ |game|
                         if game.fullscreen() {
                             "Exit fullscreen"
                         } else {
                             "Go fullscreeen"
                         }
                     }
                 }
             }
            }
        }
    };
    body().append_child(&ui)?;
  }

  let movement = Movement::new();
  movement.register_key_bindings();
  movement.register_mouse_bindings(viewport.clone());

  on_animation_frame(
    move |_| {
      if !game.paused() {
        scene.borrow_mut().physics();
        scene.borrow_mut().sync_transforms();
        let dx = movement.dx();
        let dy = movement.dy();
        player.face_viewport();
        if dx != 0 || dy != 0 {
          player.move_(dx, dy);
        }
        let pos = *scene.borrow().get_body("car").unwrap().position();
        viewport.borrow_mut().follow(pos);
      }
      renderer
        .borrow_mut()
        .render(&scene.borrow(), &viewport.borrow());
    },
    None,
  );
  Ok(())
}
