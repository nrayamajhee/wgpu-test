use log::error;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{
    window as win, Document, Element, Event, EventTarget, HtmlElement, HtmlHeadElement,
    HtmlStyleElement, Window,
};

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
    win().unwrap_or_else(|| {
        error!("No global window!");
        panic!()
    })
}

pub fn document() -> Document {
    window().document().unwrap_or_else(|| {
        error!("Window doesn't have document!");
        panic!()
    })
}

pub fn body() -> HtmlElement {
    document().body().unwrap_or_else(|| {
        error!("Document doesn't have a body!");
        panic!()
    })
}

pub fn head() -> HtmlHeadElement {
    document().head().unwrap_or_else(|| {
        error!("Document doesn't have a head!");
        panic!()
    })
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
        .expect("Can't create element")
}

pub fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
    window()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK")
}

pub fn on_animation_frame(mut closure: impl FnMut() + 'static) {
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    let closure = Closure::wrap(Box::new(move || {
        closure();
        let h = f.clone();
        request_animation_frame(h.borrow().as_ref().unwrap());
    }) as Box<dyn FnMut()>);
    *g.borrow_mut() = Some(closure);
    request_animation_frame(g.borrow().as_ref().unwrap());
}
