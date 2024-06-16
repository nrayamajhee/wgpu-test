use fluid::{Context, Signal};
use gloo_timers::callback::Timeout;
use gloo_utils::window;
use std::{cell::RefCell, rc::Rc};

use crate::{renderer::Renderer, viewport::Viewport};

pub struct Game {
  fullscreen: Rc<Signal<bool>>,
  paused: Rc<Signal<bool>>,
}
impl Game {
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
      fluid::add_event_and_forget(&document, "pointerlockchange", move |_| {
        let p = *paused.get();
        paused.set(!p);
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
        viewport.borrow_mut().resize(&renderer.borrow().canvas());
      });
    }
    Self { fullscreen, paused }
  }
  pub fn fullscreen(&self) -> bool {
    *self.fullscreen.get()
  }
  pub fn paused(&self) -> bool {
    *self.paused.get()
  }
  pub fn resume(&self, renderer: Rc<RefCell<Renderer>>, viewport: &mut Viewport) {
    if window()
      .navigator()
      .user_agent()
      .expect("No user agent in navigator")
      .contains("Chrome")
    {
      // Timeout::new(1_000, move || {
      //   renderer.borrow().canvas().request_pointer_lock();
      // })
      // .forget();
      renderer.borrow().canvas().request_pointer_lock();
    } else {
      renderer.borrow().canvas().request_pointer_lock();
    }
    viewport.unlock();
  }
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
