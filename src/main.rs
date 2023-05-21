mod mesh;
mod renderer;
mod viewport;

use gloo_console::log;
use gloo_render::request_animation_frame;
use gloo_utils::{body, document};
use mesh::Primitive;
use viewport::Viewport;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use mesh::{Geometry, Material, Mesh};
use renderer::Renderer;

async fn async_main() -> Result<(), JsValue> {
  let canvas = document().create_element("canvas")?;
  body().append_child(&canvas)?;
  let renderer = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;
  let viewport = Viewport::new();
  let geo = Geometry::from_primitive(Primitive::Plane(None));
  let mat = Material {
    tex_coords: geo
      .vertices
      .iter()
      .map(|v| [(v[0] + 1.) / 2., 1. - (v[1] + 1.) / 2.])
      .collect(),
  };
  let mesh = Mesh::new(renderer.device(), &geo, &mat);
  request_animation_frame(move |_| {
    renderer.render(vec![mesh], viewport);
  });
  Ok(())
}

fn main() {
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      log!("Couldn't spawn async main", err);
    })
  })
}
