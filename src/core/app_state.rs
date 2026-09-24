//! UI-owned application state, applied to the renderer by the frame loop.

use fluid::{Context, Signal};
use gloo_utils::window;
use std::rc::Rc;
use web_sys::HtmlCanvasElement;

use crate::core::renderer::get_window_dimension;

/// Reactive UI state: pause, fullscreen and canvas size.
///
/// The UI layer and browser listeners **write** here; nothing in the UI
/// touches the renderer. The frame loop **reads** it every frame and applies
/// changes: the [`Renderer`](crate::core::Renderer) and
/// [`Viewport`](crate::core::Viewport) resize when [`size`](AppState::size)
/// changes, and simulation stops while [`paused`](AppState::paused).
/// Signals also drive the [`PauseMenu`](crate::ui::PauseMenu) markup via
/// fluid effects.
///
/// # Events handled
/// - `pointerlockchange`: paused = pointer not locked (Esc pauses).
/// - `fullscreenchange`: syncs the fullscreen signal.
/// - `resize`: records the new window size.
pub struct AppState {
  /// Whether the document is fullscreen.
  fullscreen: Rc<Signal<bool>>,
  /// Whether the game is paused. Starts `true`.
  paused: Rc<Signal<bool>>,
  /// Target canvas size in pixels (the window's inner size).
  size: Rc<Signal<(u32, u32)>>,
  /// Canvas to lock the pointer to on resume (a DOM element, not the renderer).
  canvas: HtmlCanvasElement,
}

impl AppState {
  /// Creates the signals in `context` and registers the window/document
  /// listeners. `canvas` is the element pointer lock targets.
  pub fn new(context: &Context, canvas: HtmlCanvasElement) -> Self {
    let window = window();
    let document = Rc::new(window.document().expect("should have a document"));
    let fullscreen = context.create_signal(document.fullscreen());
    let paused = context.create_signal(true);
    let size = context.create_signal(get_window_dimension());
    {
      let paused = paused.clone();
      let document = document.clone();
      fluid::add_event_and_forget(&document.clone(), "pointerlockchange", move |_| {
        paused.set(document.pointer_lock_element().is_none());
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
      let size = size.clone();
      fluid::add_event_and_forget(&window, "resize", move |_| {
        size.set(get_window_dimension());
      });
    }
    Self {
      fullscreen,
      paused,
      size,
      canvas,
    }
  }
  /// Whether the document is fullscreen.
  pub fn fullscreen(&self) -> bool {
    *self.fullscreen.get()
  }
  /// Whether the game is paused.
  pub fn paused(&self) -> bool {
    *self.paused.get()
  }
  /// Target canvas size `(width, height)` in pixels.
  pub fn size(&self) -> (u32, u32) {
    *self.size.get()
  }
  /// Requests pointer lock on the canvas. Must run inside a user gesture
  /// (click handler); once granted, `pointerlockchange` unpauses.
  pub fn resume(&self) {
    self.canvas.request_pointer_lock();
  }
  /// Enters or exits document fullscreen and updates the signal. Must run
  /// inside a user gesture. The resulting window `resize` updates
  /// [`size`](AppState::size).
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
