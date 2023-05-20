mod mesh;
mod renderer;

use fluid::{body, document};
use mesh::Primitive;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use web_sys::HtmlCanvasElement;

use mesh::{Geometry, Mesh};
use renderer::Renderer;

async fn async_main() -> Result<(), JsValue> {
  let canvas = document()?.create_element("canvas")?;
  body()?.append_child(&canvas)?;
  let img = document()?.create_element("img")?;
  img.set_attribute("src", "img/icon.png")?;
  let renderer = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;
  let _ = Mesh::from_geometry(
    renderer.device(),
    &Geometry::from_primitive(Primitive::Cube),
  );
  Ok(())
}

fn main() {
  console_log::init().expect("Couldn't initialize console log");
  wasm_bindgen_futures::spawn_local(async move {
    async_main().await.unwrap_or_else(|err| {
      log::error!("Couldn't spawn async main {:?}", err);
    })
  })
}
