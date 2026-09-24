//! Mouse camera controls.

use std::cell::RefCell;
use std::rc::Rc;

use fluid::add_event_and_forget;
use gloo_utils::window as gloo_window;
use wasm_bindgen::JsCast;
use web_sys::{MouseEvent, WheelEvent};

use crate::core::Viewport;

/// Forwards window `wheel` events to [`Viewport::update_zoom`] and
/// `mousemove` deltas to [`Viewport::update_rot`].
/// The viewport ignores them while locked (paused).
pub fn register_mouse_bindings(viewport: Rc<RefCell<Viewport>>) {
  {
    let viewport = viewport.clone();
    add_event_and_forget(&gloo_window(), "wheel", move |e| {
      viewport
        .borrow_mut()
        .update_zoom(e.dyn_into::<WheelEvent>().unwrap().delta_y() as i32);
    });
  }
  add_event_and_forget(&gloo_window(), "mousemove", move |e| {
    let me = e.dyn_into::<MouseEvent>().unwrap();
    viewport
      .borrow_mut()
      .update_rot(me.movement_x(), me.movement_y(), 1.);
  });
}
