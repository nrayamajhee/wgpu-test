//! Shared app state bridging the UI, browser events and the frame loop.

use fluid::{Context, Signal};
use gloo_utils::window;
use std::{cell::RefCell, rc::Rc};

use crate::core::{Keyboard, Renderer, Viewport};

/// Reactive pause/fullscreen state plus shared input.
///
/// Signals drive the [`PauseMenu`](crate::ui::PauseMenu) markup,
/// and the frame loop skips simulation while [`paused`](AppState::paused).
/// The [`Keyboard`] is reachable from both via [`AppState::keyboard`].
///
/// # Events handled
/// - `pointerlockchange`: paused = pointer not locked (Esc pauses). Camera
///   input is [locked](Viewport::lock) while paused.
/// - `fullscreenchange`: syncs the fullscreen signal.
/// - `resize`: resizes the renderer and viewport projection.
pub struct AppState {
  /// Whether the document is fullscreen.
  fullscreen: Rc<Signal<bool>>,
  /// Whether the game is paused. Starts `true`.
  paused: Rc<Signal<bool>>,
  /// Held-key state shared by the UI and scene objects.
  keyboard: Keyboard,
}
impl AppState {
  /// Creates the signals in `context`, starts the [`Keyboard`] and registers
  /// the window/document listeners.
  pub fn new(
    context: &Context,
    renderer: Rc<RefCell<Renderer>>,
    viewport: Rc<RefCell<Viewport>>,
  ) -> Self {
    let window = window();
    let document = Rc::new(window.document().expect("should have a document"));
    let fullscreen = context.create_signal(document.fullscreen());
    let paused = context.create_signal(true);
    {
      let paused = paused.clone();
      let viewport = viewport.clone();
      let document = document.clone();
      fluid::add_event_and_forget(&document.clone(), "pointerlockchange", move |_| {
        let locked = document.pointer_lock_element().is_some();
        paused.set(!locked);
        let mut viewport = viewport.borrow_mut();
        if locked {
          viewport.unlock();
        } else {
          viewport.lock();
        }
      });
    }
    {
      let fullscreen = fullscreen.clone();
      let document = document.clone();
      fluid::add_event_and_forget(&window, "fullscreenchange", move |_| {
        fullscreen.set(document.fullscreen());
      });
    }
    {
      let renderer = renderer.clone();
      let viewport = viewport.clone();
      fluid::add_event_and_forget(&window, "resize", move |_| {
        renderer.borrow_mut().resize();
        viewport.borrow_mut().resize(renderer.borrow().canvas());
      });
    }
    Self {
      fullscreen,
      paused,
      keyboard: Keyboard::new(),
    }
  }
  /// Held-key state.
  pub fn keyboard(&self) -> &Keyboard {
    &self.keyboard
  }
  /// Whether the document is fullscreen.
  pub fn fullscreen(&self) -> bool {
    *self.fullscreen.get()
  }
  /// Whether the game is paused.
  pub fn paused(&self) -> bool {
    *self.paused.get()
  }
  /// Requests pointer lock on the canvas. Once granted, `pointerlockchange`
  /// unpauses and unlocks the camera.
  pub fn resume(&self, renderer: &Renderer) {
    renderer.canvas().request_pointer_lock();
  }
  /// Enters or exits document fullscreen and updates the signal.
  pub fn toggle_fullscreen(&self) {
    let document = window().document().expect("should have a document");
    if self.fullscreen() {
      document.exit_fullscreen();
      self.fullscreen.set(false);
    } else {
      document
        .document_element()
        .expect("document should have document element")
        .request_fullscreen()
        .unwrap();
      self.fullscreen.set(true);
    }
  }
}
