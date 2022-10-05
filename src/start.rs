use crate::{
  component::Component,
  dom_factory::{add_event_and_forget, add_style, body, document, on_animation_frame, window, create_el},
  renderer::Renderer,
  scene::{Primitive, Scene},
  viewport::Viewport,
};
use log::error;
use maud::html;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{KeyboardEvent, MouseEvent as WebME, WheelEvent};

#[derive(Clone)]
struct Menu {
  label: String,
  counter: usize,
}

#[derive(Debug, Clone)]
pub struct MouseEvent {
  pub dx: i32,
  pub dy: i32,
  pub ds: f64,
}

#[derive(Debug, Clone)]
pub struct Events {
  pub paused: bool,
  pub mouse: MouseEvent,
}

impl Events {
  pub fn reset(&mut self) {
    self.mouse = MouseEvent {
      dx: 0,
      dy: 0,
      ds: 0.,
    }
  }
}

use nalgebra::{UnitQuaternion, Vector3};

pub async fn start() -> Result<(), JsValue> {
  let renderer = Renderer::new().await;
  let viewport = Rc::new(RefCell::new(Viewport::new()));

  let scene = Scene::new(&renderer.device());

  let plane_geo = scene.primitive(Primitive::Plane(None));
  let mut plane = scene.mesh("Plane", plane_geo);
  //plane.model.isometry.translation.vector = [1.,0.,0.].into();
  //plane.model.isometry.rotation =
  //UnitQuaternion::from_axis_angle(&Vector3::y_axis(), std::f32::consts::PI / 2.5);

  let circle_geo = scene.primitive(Primitive::Circle(None));
  let _circle = scene.mesh("Circle", circle_geo);

  let renderer = Rc::new(RefCell::new(renderer));

  let entities = vec![plane];

  {
    let renderer = renderer.clone();
    let viewport = viewport.clone();
    add_event_and_forget(&window(), "resize", move |_| {
      viewport.borrow_mut().resize();
      renderer.borrow_mut().resize();
    });
  }

  let canvas = Rc::new(renderer.borrow().canvas());

  add_style(include_str!("css/base.css"));
  body().append_child(&canvas)?;

  let events = Rc::new(RefCell::new(Events {
    paused: true,
    mouse: MouseEvent {
      dx: 0,
      dy: 0,
      ds: 0.,
    },
  }));

  let menu = {
    let g = events.clone();
    let events = events.clone();
    let canvas = canvas.clone();
    Component::new(
      "my_menu",
      Menu {
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
            button data-on="click" data-handle="resume" {
              span {(state.label)}
            }
          }
          button data-on="click" data-handle="count" {
            span {"Count is"} span.counter data-fragment="counter" {}
          }
        }
      }
    })
    .style(include_str!("css/base.css"))
    .style(include_str!("css/menu.css"))
    .bind("shown", move |state: &Menu| {
      if g.borrow().paused {
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
      "resume",
      move |state: &Menu, _| {
        canvas.request_pointer_lock();
        events.borrow_mut().paused = false;
        None
      },
      &["shown"],
    )
    .build()
  };

  body().append_child(menu.element())?;
  let new_menu = create_el("p");
  // body().append_child(menu.element())?;
  body().append_child(&new_menu)?;

  let menu = Rc::new(menu);
  {
    let menu = menu.clone();
    let events = events.clone();
    add_event_and_forget(&document(), "pointerlockchange", move |_| {
      if document().pointer_lock_element() == None {
        document().exit_pointer_lock();
        events.borrow_mut().paused = true;
        menu.tag("shown");
      }
    });
  }
  {
    let menu = menu.clone();
    let events = events.clone();
    add_event_and_forget(&document(), "pointerlockerror", move |_| {
      // overcorrection needed for chrome
      // chrome doesn't allow switching pointer lock within a second
      events.borrow_mut().paused = true;
      menu.tag("shown");
    });
  }
  {
    let events = events.clone();
    add_event_and_forget(&canvas, "mousemove", move |e| {
      //let key = e.dyn_into::<KeyboardEvent>().unwrap().key();
      let me = e.dyn_into::<WebME>().unwrap();
      //if (key == "w") {
      //events.borrow_mut().action = {
      //}
      events.borrow_mut().mouse.dx = me.movement_x();
      events.borrow_mut().mouse.dy = me.movement_y();
    });
    add_event_and_forget(&document(), "keyup", move |e| {
      e.prevent_default();
      //log::warn!("KEYUP");
    })
  }
  {
    let events = events.clone();
    add_event_and_forget(&canvas, "wheel", move |e| {
      let me = e.dyn_into::<WheelEvent>().unwrap();
      //log::info!("{:?}", me.delta_y());
      events.borrow_mut().mouse.ds = me.delta_y();
    });
  }

  {
    let renderer = renderer.clone();
    on_animation_frame(move |dt| {
      menu.update();
      viewport.borrow_mut().update(&events.borrow(), dt);
      renderer
        .borrow_mut()
        .render(&entities, &viewport.borrow())
        .unwrap_or_else(|_| error!("Render Error!"));
      events.borrow_mut().reset();
    });
  }

  Ok(())
}
