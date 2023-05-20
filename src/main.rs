use fluid::{body, document, window};
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{gpu_buffer_usage, Gpu, GpuBuffer, GpuAdapter, GpuDevice, GpuQueue, HtmlCanvasElement};

struct Renderer {
  canvas: HtmlCanvasElement,
  device: GpuDevice,
  queue: GpuQueue,
  // vertex_buffer: GpuBuffer,
  // index_buffer: GpuBuffer,
}

struct GpuMesh {
  vertext_buffer: GpuBuffer,
  index_buffer: GpuBuffer,
}

// impl GpuMesh {
//     fn new(
// }

impl Renderer {
  pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
    let gpu = window()?.navigator().gpu();
    let adapter = JsFuture::from(gpu.request_adapter())
      .await?
      .dyn_into::<GpuAdapter>()?;
    let device = JsFuture::from(adapter.request_device())
      .await?
      .dyn_into::<GpuDevice>()?;
    let queue = device.queue();
    // let vertext_buffer = device.create_buffer(GpuBufferDescriptor::new(gpu_buffer_usage::VERTEX))
    web_sys::console::log_1(&queue);
    Ok(Self {
      canvas,
      device,
      queue,
      // vertex_buffer
    })
  }
}

async fn async_main() -> Result<(), JsValue> {
  let canvas = document()?.create_element("canvas")?;
  body()?.append_child(&canvas)?;
  let _ = Renderer::new(canvas.dyn_into::<HtmlCanvasElement>()?).await?;
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
