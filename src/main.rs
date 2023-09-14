mod mesh;
mod renderer;
mod viewport;

use gloo_console::log;
use gloo_timers::callback::Timeout;
use gloo_utils::{body, document, window};
use mesh::Primitive;
use viewport::Viewport;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, Performance};

use mesh::{Geometry, Material, Mesh};
use renderer::Renderer;
use std::cell::RefCell;
use std::rc::Rc;

fn main() {
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      log!("Couldn't spawn async main", err);
    })
  })
}

async fn async_main() -> Result<(), JsValue> {
  let canvas = document().create_element("canvas")?;
  body().append_child(&canvas)?;
  let mut renderer = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;
  let viewport = Viewport::new(renderer.canvas());

  let geo = Geometry::from_primitive(Primitive::Cube);
  let mat = Material {
    vertex_colors: geo.vertices.iter().map(|_| [1., 1., 1.]).collect(),
    tex_coords: geo
      .vertices
      .iter()
      .map(|v| [(v[0] + 1.) / 2., 1. - (v[1] + 1.) / 2.])
      .collect(),
  };
  let mesh = Mesh::new(renderer.device(), &geo, &mat);
  let geo = Geometry::from_primitive(Primitive::Plane(None));
  let mat = Material {
    vertex_colors: geo.vertices.iter().map(|_| [1., 0., 1.]).collect(),
    tex_coords: vec![],
  };
  let mesh2 = Mesh::new(renderer.device(), &geo, &mat);

  let meshes = vec![mesh, mesh2];

  on_animation_frame(
    move |_| {
      renderer.render(&meshes, &viewport);
    },
    None,
  );
  Ok(())
}

fn now() -> f64 {
  window().performance().unwrap().now()
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) -> i32 {
  window()
    .request_animation_frame(f.as_ref().unchecked_ref())
    .expect("should register `requestAnimationFrame` OK")
}

pub fn on_animation_frame(mut closure: impl FnMut(f64) + 'static, fps: Option<f64>) {
  let t = Rc::new(RefCell::new(0.));
  let f = Rc::new(RefCell::new(None));
  let g = f.clone();
  let then = t.clone();
  let closure = Closure::wrap(Box::new(move || {
    let mut then = then.borrow_mut();
    let delta = now() - *then;
    closure(delta);
    *then = now();
    let h = f.clone();
    let next_frame = move || {
      request_animation_frame(h.borrow().as_ref().unwrap());
    };
    if let Some(fps) = fps {
      Timeout::new(((1000. / fps) - delta) as u32, next_frame).forget();
    } else {
      next_frame();
    };
  }) as Box<dyn FnMut()>);
  *g.borrow_mut() = Some(closure);
  *t.borrow_mut() = now();
  request_animation_frame(g.borrow().as_ref().unwrap());
}
