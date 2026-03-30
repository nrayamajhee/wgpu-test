use std::cell::RefCell;
use std::rc::Rc;

use fluid::add_event_and_forget;
use gloo_utils::window as gloo_window;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, MouseEvent, WheelEvent};

use crate::Viewport;

#[derive(Debug)]
struct State {
  dx: isize,
  dy: isize,
}

pub struct Movement {
  state: Rc<RefCell<State>>,
}

impl Movement {
  pub fn new() -> Self {
    let state = Rc::new(RefCell::new(State { dx: 0, dy: 0 }));
    Self { state }
  }

  pub fn register_key_bindings(&self) {
    {
      let state = self.state.clone();
      add_event_and_forget(&gloo_window(), "keydown", move |e| {
        let key = e.dyn_into::<KeyboardEvent>().unwrap().key();
        let mut s = state.borrow_mut();
        match key.as_str() {
          "w" => s.dy = Self::next_delta(s.dy, 1),
          "s" => s.dy = Self::next_delta(s.dy, -1),
          "a" => s.dx = Self::next_delta(s.dx, -1),
          "d" => s.dx = Self::next_delta(s.dx, 1),
          _ => {}
        }
      });
    }
    {
      let state = self.state.clone();
      add_event_and_forget(&gloo_window(), "keyup", move |e| {
        let key = e.dyn_into::<KeyboardEvent>().unwrap().key();
        let mut s = state.borrow_mut();
        match key.as_str() {
          "w" => s.dy = Self::next_delta(s.dy, -1),
          "s" => s.dy = Self::next_delta(s.dy, 1),
          "a" => s.dx = Self::next_delta(s.dx, 1),
          "d" => s.dx = Self::next_delta(s.dx, -1),
          _ => {}
        }
      });
    }
  }

  pub fn dx(&self) -> isize {
    self.state.borrow().dx
  }

  pub fn dy(&self) -> isize {
    self.state.borrow().dy
  }

  fn next_delta(prev: isize, next: isize) -> isize {
    let val = prev + next;
    if val >= 1 {
      1
    } else if val <= -1 {
      -1
    } else {
      0
    }
  }

  pub fn register_mouse_bindings(&self, viewport: Rc<RefCell<Viewport>>) {
    {
      let viewport = viewport.clone();
      add_event_and_forget(&gloo_window(), "wheel", move |e| {
        viewport
          .borrow_mut()
          .update_zoom(e.dyn_into::<WheelEvent>().unwrap().delta_y() as i32);
      });
    }
    {
      add_event_and_forget(&gloo_window(), "mousemove", move |e| {
        let me = e.dyn_into::<MouseEvent>().unwrap();
        viewport
          .borrow_mut()
          .update_rot(me.movement_x(), me.movement_y(), 1.);
      });
    }
  }
}
