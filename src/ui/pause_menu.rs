//! Pause overlay UI. See [`PauseMenu`].

use std::cell::RefCell;
use std::rc::Rc;

use fluid::Context;
use fluid_macro::html;
use wasm_bindgen::prelude::*;
use web_sys::Element;

use crate::core::{AppState, Renderer};

/// Overlay with **Resume**, **fullscreen toggle** and **Documentation** buttons.
///
/// The docs link is relative (`docs/`), so it resolves under any Trunk
/// `public_url`; the docs are built by `scripts/build-docs.nu`.
///
/// Shown (`.overlay.shown`) while [`AppState::paused`] is true.
pub struct PauseMenu;

impl PauseMenu {
  /// Builds the overlay element; append it to the DOM to display it.
  ///
  /// `ctx` and `app_state` names are required by `html!` reactive blocks.
  ///
  /// # Errors
  /// If DOM element creation fails.
  pub fn new(
    ctx: &Context,
    app_state: &Rc<AppState>,
    renderer: &Rc<RefCell<Renderer>>,
  ) -> Result<Element, JsValue> {
    let app_state = app_state.clone();
    let renderer = renderer.clone();
    let on_resume = app_state.clone();
    let on_fullscreen = app_state.clone();
    let ui = html! {
        div class=#{ |app_state| &format!("overlay {}", if app_state.paused() { "shown" } else { "" }) } {
            div class="pause-menu" {
             h1 { "Pause Menu" }
             div class="buttons" {
                 button
                 class="resume-btn"
                 @click={ move |_| {
                     on_resume.resume(&renderer.borrow())
                 } }
                 { "Resume" }
                 button
                 @click={ move |_| {
                     on_fullscreen.toggle_fullscreen();
                 } }
                 {
                     #{ |app_state|
                         if app_state.fullscreen() {
                             "Exit fullscreen"
                         } else {
                             "Go fullscreen"
                         }
                     }
                 }
                 a
                 class="button"
                 href="docs/"
                 target="_blank"
                 rel="noopener"
                 { "Documentation" }
             }
            }
        }
    };
    Ok(ui)
  }
}
