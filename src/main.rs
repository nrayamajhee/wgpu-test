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
//!   - [`Viewport`] is an orbit camera, driven by mouse input each frame.
//!   - [`EventQueue`] and [`InputState`] carry scene input
//!     (see [Data flow](#data-flow)).
//!   - [`AppState`] holds pause, fullscreen and canvas size, written by the UI
//!     and applied to the renderer by the frame loop.
//! - [`lights`] — sun, point and ambient lights, attached to scene nodes.
//! - [`things`] — scene content builders ([`World`], [`Device`], a playable piano).
//! - [`game`] — player and car (not wired in yet).
//! - [`ui`] — DOM overlays ([`PauseMenu`]).
//! - [`utils`] — small helpers.
//!
//! ## Data flow
//!
//! ```text
//! browser events ──► EventQueue ──(drained per frame)──► InputState
//!                                                           │
//!   run_loop: Viewport::update, Device::update, physics ◄───┘ ──► render
//!
//! UI (fluid) / browser ──► AppState (paused, fullscreen, size)
//!                              │ read every frame
//!   run_loop: Renderer::fit, Viewport::fit, pause gating ◄──┘
//! ```
//!
//! [`async_main`] fully configures the renderer, viewport and scene first.
//! The UI only writes to [`AppState`] and never touches the renderer; the
//! frame loop owns the renderer, scene, viewport and input state.
//!
//! ## Frame loop
//!
//! See [`run_loop`]: apply queued events → apply [`AppState`] (resize the
//! renderer and camera) →
//! (if not paused) orbit/zoom, update the [`Device`], step physics, follow
//! the device → render.

// Content constructors like `World::new` intentionally return a `Group`.
#![allow(clippy::new_ret_no_self)]

mod core;
#[allow(dead_code)]
mod game;
mod lights;
mod things;
mod ui;
mod utils;

use crate::core::{AppState, EventQueue, InputState, Renderer, Scene, Viewport};
use things::{Device, World};
use ui::PauseMenu;

use fluid::{on_animation_frame, Context};
use gloo_console::log;
use gloo_utils::body;
use wasm_bindgen::prelude::*;

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

  let events = EventQueue::new();
  events.register();

  // Mount UI
  let ctx = Context::new();
  let app_state = Rc::new(AppState::new(&ctx, renderer.canvas().clone()));
  let pause_menu = PauseMenu::new(&ctx, &app_state)?;
  body().append_child(renderer.canvas())?;
  body().append_child(&pause_menu)?;

  // Run
  run_loop(scene, viewport, events, renderer, app_state);
  Ok(())
}

/// Starts the per-frame loop, which owns the `renderer`, `scene`, `viewport`
/// and input.
///
/// Each frame: drain `events` into [`InputState`], apply [`AppState`] (the
/// renderer and camera resize when its size changed), and render. While not paused, also orbit/zoom the camera, update
/// the [`Device`], step physics and follow the device.
///
/// Input is applied even while paused, so keys released during the pause
/// aren't stuck down afterwards.
fn run_loop(
  mut scene: Scene,
  mut viewport: Viewport,
  events: EventQueue,
  mut renderer: Renderer,
  app_state: Rc<AppState>,
) {
  let mut input = InputState::default();
  on_animation_frame(
    move |_| {
      input.apply(events.drain());
      let paused = app_state.paused();
      let size = app_state.size();
      renderer.fit(size);
      viewport.fit(size);
      viewport.update(&input, paused);
      if !paused {
        Device::update(&mut scene, &input);
        scene.physics();
        scene.sync_transforms();
        if let Some(target) = scene.world_transform(Device::NODE) {
          viewport.follow(target.isometry);
        }
      }
      renderer.render(&scene, &viewport);
    },
    None,
  );
}
