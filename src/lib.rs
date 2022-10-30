#[wasm_bindgen]
extern "C" {
  #[wasm_bindgen(js_namespace = console, js_name = log)]
  fn logv(x: &JsValue);
}

mod component;
pub mod dom_factory;
pub mod pool;
mod renderer;
mod mesh;
mod scene;
mod start;
// mod viewport;

#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use crate::pool::RayonWorkers;
use log::Level;

use rayon::prelude::*;
use wasm_bindgen::prelude::*;
use futures_channel::oneshot;

#[wasm_bindgen]
pub async fn run() -> Result<(), JsValue> {
  console_error_panic_hook::set_once();
  console_log::init_with_level(Level::Debug).unwrap_or_else(|e| {
    crate::logv(&JsValue::from_str(&format!(
      "Couldn't initialize logger:\n{}",
      e,
    )))
  });

  // let workers = RayonWorkers::new(None);
  // let (tx, rx) = oneshot::channel::<f64>();
  // workers.run(move || {
  //   let sum = (1..=1_000_000)
  //     .into_par_iter()
  //     .map(|e| f64::sqrt(e as f64))
  //     .sum::<f64>();
  //   drop(tx.send(sum));
  // });
  // let sum = rx.await.unwrap();
  // log::info!("Raypon par iter test: {:?}", sum);
  start::start().await
}
