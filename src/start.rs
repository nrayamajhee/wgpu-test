use crate::{
  component::Component,
  dom_factory::{add_event_and_forget, add_style, body, document, on_animation_frame, window},
  renderer::Renderer,
  scene::{Primitive, Scene},
  viewport::Viewport,
};
use log::error;
use maud::html;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;

#[derive(Clone)]
struct Menu {
  paused: bool,
  label: String,
  counter: usize,
}

pub async fn start() -> Result<(), JsValue> {
  let renderer = Renderer::new().await;
  let scene = Scene::new(&renderer.device());
  let viewport = Rc::new(Viewport::new());

  let plane_geo = scene.primitive(Primitive::Plane(None));
  let plane = scene.mesh("Plane", plane_geo);

  let circle_geo = scene.primitive(Primitive::Circle(None));
  let _circle = scene.mesh("Circle", circle_geo);

  let shared_renderer = Rc::new(RefCell::new(renderer));
  let entities = Rc::new(vec![plane]);

  let _r = shared_renderer.clone();
  let e = entities.clone();
  let v = viewport.clone();

  let r = shared_renderer.clone();
  add_event_and_forget(&window(), "resize", move |_| {
    r.borrow_mut().resize();
  });

  let canvas = shared_renderer.borrow().canvas();

  add_style(include_str!("css/base.css"));
  body().append_child(&canvas)?;

  let c = canvas.clone();
  let menu = Component::new(
    "my_menu",
    Menu {
      paused: true,
      counter: 0,
      label: "Resume".to_string(),
    },
  )
  .markup(|state| {
    html! {
      main {}
      .background data-attrib="class" data-bind="shown" {
        div {
          h2 {"Main Menu"}
          button data-on="click" data-handle="capture-cursor" {
            span {(state.label)}
          }
        }
        button data-on="click" data-handle="count" {
          span {"Count is"} span.counter data-fragment="counter" {}
        }
      }
    }
  })
  .style(include_str!("css/menu.css"))
  .bind("shown", |state: &Menu| {
    if state.paused {
      "background shown".to_string()
    } else {
      "background".to_string()
    }
  })
  .fragment("counter", |state: &Menu| {
    html! {
      (state.counter)
    }
  })
  .handler(
    "count",
    move |state: &Menu, _| {
      Some(Menu {
        counter: state.counter + 1,
        ..state.clone()
      })
    },
    &["counter"],
  )
  .handler(
    "capture-cursor",
    move |state: &Menu, _| {
      c.request_pointer_lock();
      Some(Menu {
        paused: false,
        ..state.clone()
      })
    },
    &["shown"],
  )
  .build();

  body().append_child(menu.element())?;

  let menu = Rc::new(menu);
  let m = menu.clone();
  add_event_and_forget(&document(), "pointerlockchange", move |_| {
    if document().pointer_lock_element() == None {
      m.set_state(Menu {
        paused: true,
        ..m.state()
      });
      m.tag("shown");
    }
  });

  let r = shared_renderer.clone();
  let m = menu.clone();
  on_animation_frame(move || {
    r.borrow_mut()
      .render(e.as_ref(), v.as_ref())
      .unwrap_or_else(|_| error!("Render Error!"));
    m.update();
  });

  Ok(())
}
