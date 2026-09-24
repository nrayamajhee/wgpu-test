//! Scene input: browser events → [`EventQueue`] → [`InputState`].
//!
//! Listeners only record [`Event`]s. The frame loop drains the queue once
//! per frame and folds the events into [`InputState`] in order, which scene
//! objects then read. Ordering is preserved, so a key pressed and released
//! within one frame still shows up as [`just_pressed`](InputState::just_pressed).
//!
//! UI concerns (pause, fullscreen, resize) stay in
//! [`AppState`](crate::core::AppState) and don't go through here.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// A scene-relevant browser event.
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
  /// Key went down, by `KeyboardEvent.code` (auto-repeats are not recorded).
  KeyDown(String),
  /// Key went up, by `KeyboardEvent.code`.
  KeyUp(String),
  /// Relative mouse movement in pixels (`movementX`, `movementY`).
  MouseMove(i32, i32),
  /// Wheel scroll; positive = scroll down / zoom out.
  Wheel(f32),
  /// Window lost focus; every held key is released.
  Blur,
}

/// Events recorded since the last frame. Cloning shares the same queue.
#[derive(Clone, Default)]
pub struct EventQueue {
  /// Pending events, oldest first.
  events: Rc<RefCell<Vec<Event>>>,
}

impl EventQueue {
  /// Creates an empty queue with no listeners.
  pub fn new() -> Self {
    Self::default()
  }

  /// Records an event.
  pub fn push(&self, event: Event) {
    self.events.borrow_mut().push(event);
  }

  /// Removes and returns all pending events, oldest first.
  pub fn drain(&self) -> Vec<Event> {
    std::mem::take(&mut *self.events.borrow_mut())
  }

  /// Installs the window listeners that feed this queue: `keydown`, `keyup`,
  /// `blur`, `mousemove` and `wheel`. Call once.
  pub fn register(&self) {
    use fluid::add_event_and_forget;
    use gloo_utils::window;
    use wasm_bindgen::JsCast;
    use web_sys::{KeyboardEvent, MouseEvent, WheelEvent};

    let listen = |name: &str, to_event: fn(web_sys::Event) -> Option<Event>| {
      let queue = self.clone();
      add_event_and_forget(&window(), name, move |e| {
        if let Some(event) = to_event(e) {
          queue.push(event);
        }
      });
    };
    listen("keydown", |e| {
      let e = e.dyn_into::<KeyboardEvent>().ok()?;
      (!e.repeat()).then(|| Event::KeyDown(e.code()))
    });
    listen("keyup", |e| {
      Some(Event::KeyUp(e.dyn_into::<KeyboardEvent>().ok()?.code()))
    });
    listen("blur", |_| Some(Event::Blur));
    listen("mousemove", |e| {
      let e = e.dyn_into::<MouseEvent>().ok()?;
      Some(Event::MouseMove(e.movement_x(), e.movement_y()))
    });
    listen("wheel", |e| {
      Some(Event::Wheel(e.dyn_into::<WheelEvent>().ok()?.delta_y() as f32))
    });
  }
}

/// Input as of the current frame, built by [`InputState::apply`].
///
/// Per-frame fields (`pressed`, `released`, `mouse_delta`, `wheel`) are
/// reset at the start of each `apply`; `held` persists across frames.
#[derive(Default, Debug)]
pub struct InputState {
  /// Keys currently down.
  held: HashSet<String>,
  /// Keys that went down this frame (even if already released again).
  pressed: HashSet<String>,
  /// Keys that went up this frame.
  released: HashSet<String>,
  /// Summed mouse movement this frame, in pixels.
  mouse_delta: (i32, i32),
  /// Summed wheel scroll this frame.
  wheel: f32,
}

impl InputState {
  /// Starts a new frame and folds `events` in, in order.
  pub fn apply(&mut self, events: impl IntoIterator<Item = Event>) {
    self.pressed.clear();
    self.released.clear();
    self.mouse_delta = (0, 0);
    self.wheel = 0.;
    for event in events {
      match event {
        Event::KeyDown(code) => {
          self.held.insert(code.clone());
          self.pressed.insert(code);
        }
        Event::KeyUp(code) => {
          self.held.remove(&code);
          self.released.insert(code);
        }
        Event::MouseMove(dx, dy) => {
          self.mouse_delta.0 += dx;
          self.mouse_delta.1 += dy;
        }
        Event::Wheel(delta) => self.wheel += delta,
        Event::Blur => self.released.extend(self.held.drain()),
      }
    }
  }

  /// Whether key `code` is down at the end of this frame.
  pub fn is_held(&self, code: &str) -> bool {
    self.held.contains(code)
  }

  /// Whether key `code` went down during this frame.
  pub fn just_pressed(&self, code: &str) -> bool {
    self.pressed.contains(code)
  }

  /// Whether key `code` went up during this frame.
  pub fn just_released(&self, code: &str) -> bool {
    self.released.contains(code)
  }

  /// Whether key `code` should count as active this frame: held, or tapped
  /// (pressed and released) within it.
  pub fn is_active(&self, code: &str) -> bool {
    self.is_held(code) || self.just_pressed(code)
  }

  /// Whether either Shift key is held.
  pub fn shift(&self) -> bool {
    self.is_held("ShiftLeft") || self.is_held("ShiftRight")
  }

  /// Summed mouse movement this frame, in pixels.
  pub fn mouse_delta(&self) -> (i32, i32) {
    self.mouse_delta
  }

  /// Summed wheel scroll this frame; positive = zoom out.
  pub fn wheel(&self) -> f32 {
    self.wheel
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn key(code: &str) -> String {
    code.to_owned()
  }

  #[test]
  fn tap_within_one_frame_is_seen() {
    let mut input = InputState::default();
    input.apply([Event::KeyDown(key("KeyA")), Event::KeyUp(key("KeyA"))]);
    assert!(!input.is_held("KeyA"));
    assert!(input.just_pressed("KeyA"));
    assert!(input.is_active("KeyA"));
    input.apply([]);
    assert!(!input.is_active("KeyA"));
  }

  #[test]
  fn held_persists_and_blur_releases() {
    let mut input = InputState::default();
    input.apply([Event::KeyDown(key("ShiftLeft"))]);
    input.apply([]);
    assert!(input.shift());
    assert!(!input.just_pressed("ShiftLeft"));
    input.apply([Event::Blur]);
    assert!(!input.shift());
    assert!(input.just_released("ShiftLeft"));
  }

  #[test]
  fn mouse_and_wheel_sum_per_frame() {
    let mut input = InputState::default();
    input.apply([Event::MouseMove(1, 2), Event::MouseMove(3, -1), Event::Wheel(5.)]);
    assert_eq!(input.mouse_delta(), (4, 1));
    assert_eq!(input.wheel(), 5.);
    input.apply([]);
    assert_eq!(input.mouse_delta(), (0, 0));
    assert_eq!(input.wheel(), 0.);
  }
}
