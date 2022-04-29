#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => (crate::log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
extern "C" {
    /// External binding to the js console.log method. You probably want to use console_log!() instead
    /// of this.
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
    #[wasm_bindgen]
    pub fn alert(s: &str);
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn logv(x: &JsValue);
}

mod dom_factory;
mod pool;
mod renderer;
mod scene;
mod start;
mod viewport;
pub use dom_factory::*;
pub use viewport::*;
use log::Level;
pub use pool::*;
pub use renderer::*;
pub use scene::*;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;

// When the `wee_alloc` feature is enabled, use `wee_alloc` as the global
// allocator.
#[cfg(feature = "wee_alloc")]
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

use crate::WorkerPool;
use rayon::ThreadPool;
use std::sync::Arc;

pub struct RayonWorkers {
    concurrency: usize,
    worker_pool: WorkerPool,
    rayon_pool: Arc<ThreadPool>,
}

impl RayonWorkers {
    pub fn new(concurrency: Option<usize>) -> Self {
        let concurrency = if let Some(con) = concurrency {
            con
        } else {
            if let Some(window) = web_sys::window() {
                window.navigator().hardware_concurrency() as usize
            } else {
                crate::log("Couldn't get window!");
                2
            }
        };
        let worker_pool = WorkerPool::new(concurrency).unwrap();
        let rayon_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(concurrency)
            .spawn_handler(|thread| Ok(worker_pool.run(|| thread.run()).unwrap()))
            .build()
            .unwrap();
        let rayon_pool = Arc::new(rayon_pool);
        Self {
            concurrency,
            worker_pool,
            rayon_pool,
        }
    }
    pub fn concurrency(&self) -> usize {
        self.concurrency
    }
    pub fn run<F: 'static + Fn() + Sync + Send>(&self, closure: F) {
        let r_p = self.rayon_pool.clone();
        self.worker_pool.run(move || {
            r_p.install(|| {
                closure();
            });
        });
    }
}

#[wasm_bindgen]
pub async fn run() -> Result<(), JsValue> {
    #[cfg(feature = "console_error_panic_hook")]
    console_error_panic_hook::set_once();
    console_log::init_with_level(Level::Debug);
    start::start().await
}
