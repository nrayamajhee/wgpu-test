//! # wgpu-test
//!
//! A WebGPU + Rapier 3D sandbox compiled to WebAssembly.
//!
//! ## Modules
//!
//! - [`core`] — engine:
//!   - [`Renderer`] owns the WebGPU device, PBR and skybox pipelines and canvas,
//!     and draws a [`Scene`] (see `docs/shaders.md`).
//!   - [`Scene`] is a node tree of transforms, meshes, lights and Rapier physics objects.
//!   - [`Group`](core::Group) is detached scene content, inserted with [`Scene::add_group`].
//!   - [`Viewport`] is an orbit camera, driven by [`core::camera_controls::register_mouse_bindings`].
//!   - [`AppState`] holds reactive pause/fullscreen state shared by the UI and frame loop.
//!   - [`Keyboard`](core::Keyboard) tracks held keys; shared via [`AppState::keyboard`].
//! - [`lights`] — sun, point and ambient lights, attached to scene nodes.
//! - [`things`] — scene content builders ([`World`], [`Device`], a playable piano).
//! - [`game`] — player and car (not wired in yet).
//! - [`ui`] — DOM overlays ([`PauseMenu`]).
//! - [`utils`] — small helpers.
//!
//! ## Startup
//!
//! [`async_main`] fully configures the renderer, viewport and scene first, then
//! wraps the shared parts in `Rc<RefCell<_>>` for the UI and frame loop.
//!
//! ## Frame loop
//!
//! See [`run_loop`]. While not paused: step physics → sync body transforms →
//! update the [`Device`] from held keys → follow it with the camera. Always render.

// Content constructors like `World::new` intentionally return a `Group`.
#![allow(clippy::new_ret_no_self)]

mod core;
#[allow(dead_code)]
mod game;
mod lights;
mod things;
mod ui;
mod utils;

use crate::core::{camera_controls, AppState, Renderer, Scene, Viewport};
use things::{Device, World};
use ui::PauseMenu;

use fluid::{on_animation_frame, Context};
use gloo_console::log;
use gloo_utils::body;
use wasm_bindgen::prelude::*;

use std::cell::RefCell;
use std::rc::Rc;

/// Wasm entry point: spawns [`async_main`] on the browser event loop and logs any error.
fn main() {
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      log!("Couldn't spawn async main", err);
    })
  })
}

/// Builds and configures everything, mounts the canvas and UI, then starts
/// [`run_loop`].
///
/// # Errors
/// Fails if WebGPU is unavailable, an asset fails to load, or DOM insertion fails.
async fn async_main() -> Result<(), JsValue> {
  // Configure
  let renderer = Renderer::new().await?;

  let mut viewport = Viewport::new(renderer.canvas());
  viewport.set_min_distance(1.);
  viewport.set_max_distance(World::RADIUS);

  let mut scene = Scene::new();
  scene.add_group(World::new(&renderer).await?)?;
  scene.add_group(Device::new(&renderer).await?)?;

  // Share
  let renderer = Rc::new(RefCell::new(renderer));
  let viewport = Rc::new(RefCell::new(viewport));

  // Mount UI
  let ctx = Context::new();
  let app_state = Rc::new(AppState::new(&ctx, renderer.clone(), viewport.clone()));
  let pause_menu = PauseMenu::new(&ctx, &app_state, &renderer)?;
  body().append_child(renderer.borrow().canvas())?;
  body().append_child(&pause_menu)?;
  camera_controls::register_mouse_bindings(viewport.clone());

  // Run
  run_loop(scene, renderer, viewport, app_state);
  Ok(())
}

/// Starts the per-frame loop, which takes ownership of the `scene`.
///
/// While not paused: steps physics, syncs body transforms, updates the
/// [`Device`] from held keys and points the camera at it. Renders every frame.
fn run_loop(
  mut scene: Scene,
  renderer: Rc<RefCell<Renderer>>,
  viewport: Rc<RefCell<Viewport>>,
  app_state: Rc<AppState>,
) {
  on_animation_frame(
    move |_| {
      if !app_state.paused() {
        scene.physics();
        scene.sync_transforms();
        Device::update(&mut scene, app_state.keyboard());
        if let Some(target) = scene.world_transform(Device::NODE) {
          viewport.borrow_mut().follow(target.isometry);
        }
      }
      renderer.borrow_mut().render(&scene, &viewport.borrow());
    },
    None,
  );
}
