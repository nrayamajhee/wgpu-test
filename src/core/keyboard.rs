//! Held-key state. See [`Keyboard`].

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fluid::add_event_and_forget;
use gloo_utils::window;
use wasm_bindgen::JsCast;
use web_sys::KeyboardEvent;

/// Pressed/released state of every key seen so far, keyed by
/// [`KeyboardEvent.code`](https://developer.mozilla.org/docs/Web/API/KeyboardEvent/code)
/// (e.g. `"KeyA"`, `"Semicolon"`).
///
/// `code` is the physical key position, so state is unaffected by Shift,
/// Caps Lock or the user's keyboard layout.
///
/// Cloning is cheap and shares the same state, so the UI and scene objects
/// can each hold one.
///
/// # Events handled (on `window`)
/// - `keydown`: marks the key pressed (auto-repeat events are ignored).
/// - `keyup`: marks the key released.
/// - `blur`: releases all keys, since `keyup` is never delivered once the
///   window loses focus (e.g. Alt+Tab while holding a key).
#[derive(Clone)]
pub struct Keyboard {
  /// `code` → `true` while held, `false` once released.
  pressed: Rc<RefCell<HashMap<String, bool>>>,
}

impl Keyboard {
  /// Starts tracking keys by registering window listeners.
  pub fn new() -> Self {
    let keyboard = Self {
      pressed: Rc::new(RefCell::new(HashMap::new())),
    };
    for (event, pressed) in [("keydown", true), ("keyup", false)] {
      let keyboard = keyboard.clone();
      add_event_and_forget(&window(), event, move |e| {
        if let Ok(e) = e.dyn_into::<KeyboardEvent>() {
          if !e.repeat() {
            keyboard.set(e.code(), pressed);
          }
        }
      });
    }
    {
      let keyboard = keyboard.clone();
      add_event_and_forget(&window(), "blur", move |_| keyboard.release_all());
    }
    keyboard
  }

  /// Whether the key with `code` is currently held.
  pub fn is_pressed(&self, code: &str) -> bool {
    self.pressed.borrow().get(code).copied().unwrap_or(false)
  }

  /// Whether either Shift key is held.
  pub fn shift(&self) -> bool {
    self.is_pressed("ShiftLeft") || self.is_pressed("ShiftRight")
  }

  /// Records the state of key `code`.
  fn set(&self, code: String, pressed: bool) {
    self.pressed.borrow_mut().insert(code, pressed);
  }

  /// Marks every key as released.
  fn release_all(&self) {
    self
      .pressed
      .borrow_mut()
      .values_mut()
      .for_each(|p| *p = false);
  }
}
