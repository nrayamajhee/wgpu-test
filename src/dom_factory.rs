use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
  window as win, Document, Element, Event, EventTarget, HtmlElement, HtmlHeadElement,
  HtmlStyleElement, NodeList, ShadowRoot, ShadowRootInit, ShadowRootMode, Window,
};

#[macro_export]
macro_rules! throw {(
  $($t:tt)*
) => {{
  log::error!($($t)*);
  panic!()
}}}

pub fn now() -> f64 {
  window()
    .performance()
    .expect("Performance isn't available")
    .now()
}

pub fn add_event(
  target: &EventTarget,
  event_type: &str,
  event_listener: impl FnMut(Event) + 'static,
) -> Closure<dyn FnMut(Event)> {
  let cl = Closure::wrap(Box::new(event_listener) as Box<dyn FnMut(_)>);
  target
    .add_event_listener_with_callback(event_type, cl.as_ref().unchecked_ref())
    .unwrap();
  cl
}

pub fn add_event_and_forget(
  target: &EventTarget,
  event_type: &str,
  event_listener: impl FnMut(Event) + 'static,
) {
  add_event(target, event_type, event_listener).forget();
}

pub fn window() -> Window {
  win().unwrap_or_else(|| throw!("No global window!"))
}

pub fn document() -> Document {
  window()
    .document()
    .unwrap_or_else(|| throw!("Window doesn't have document!"))
}

pub fn body() -> HtmlElement {
  document()
    .body()
    .unwrap_or_else(|| throw!("Document doesn't have a body!"))
}

pub fn head() -> HtmlHeadElement {
  document()
    .head()
    .unwrap_or_else(|| throw!("Document doesn't have a head!"))
}

pub fn add_style(style: &str) {
  let style_el = create_el("style").dyn_into::<HtmlStyleElement>().unwrap();
  style_el.set_type("text/css");
  style_el.set_inner_html(style);
  head().append_child(&style_el).unwrap();
}

pub fn create_el(tag: &str) -> Element {
  document()
    .create_element(tag)
    .unwrap_or_else(|e| throw!("Can't create element with tagname {}!\n{:?}", tag, e))
}

pub fn create_el_w_attrs(name: &str, attributes: &[(&str, &str)]) -> Element {
  let el = create_el(name);
  for (key, value) in attributes.iter() {
    el.set_attribute(key, value)
      .unwrap_or_else(|_| throw!("Can't set attributes {} {} to {}", key, value, name))
  }
  el
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
  window()
    .request_animation_frame(f.as_ref().unchecked_ref())
    .unwrap_or_else(|e| throw!("Can't request animation frame {:?}", e))
}

pub fn on_animation_frame(mut closure: impl FnMut(f64) + 'static) {
  let f = Rc::new(RefCell::new(None));
  let g = f.clone();
  let mut then = now();
  let closure = Closure::wrap(Box::new(move || {
    let dt = now() - then;
    closure(dt);
    then = now();
    request_animation_frame(f.borrow().as_ref().unwrap());
  }) as Box<dyn FnMut()>);
  *g.borrow_mut() = Some(closure);
  request_animation_frame(g.borrow().as_ref().unwrap());
}

pub fn query_els(selector: &str) -> NodeList {
  document()
    .query_selector_all(selector)
    .unwrap_or_else(|e| throw!("No element matches selector: {}!\n{:?}", selector, e))
}

pub fn create_shadow(name: &str) -> (Element, ShadowRoot) {
  let name = if name.contains('-') {
    name.into()
  } else {
    format!("component-{}", name)
  };
  let el = create_el(&name);
  let shadow = el
    .attach_shadow(&ShadowRootInit::new(ShadowRootMode::Open))
    .unwrap_or_else(|e| throw!("Can't attach shadow root:\n{:?}", e));
  (el, shadow)
}
