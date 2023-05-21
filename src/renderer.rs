use crate::mesh::Mesh;
use crate::viewport::Viewport;
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::Array;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::GpuTexture;
use web_sys::{
  gpu_texture_usage, GpuAdapter, GpuCanvasAlphaMode, GpuCanvasConfiguration, GpuCanvasContext,
  GpuColorTargetState, GpuCompareFunction, GpuCullMode, GpuDepthStencilState, GpuDevice,
  GpuFragmentState, GpuFrontFace, GpuLoadOp, GpuPrimitiveState, GpuPrimitiveTopology, GpuQueue,
  GpuRenderPassColorAttachment, GpuRenderPassDescriptor, GpuRenderPipeline,
  GpuRenderPipelineDescriptor, GpuShaderModuleDescriptor, GpuStoreOp, GpuTextureDescriptor,
  GpuTextureFormat, GpuVertexAttribute, GpuVertexBufferLayout, GpuVertexFormat, GpuVertexState,
  HtmlCanvasElement,
};

pub fn get_window_dimension() -> (u32, u32) {
  let window = window();
  (
    window
      .inner_width()
      .expect("Window has no width")
      .as_f64()
      .expect("Width isn't f64") as u32,
    window
      .inner_height()
      .expect("Window has no height")
      .as_f64()
      .expect("Height isn't f64") as u32,
  )
}

pub fn iter_to_array<T>(iterable: impl IntoIterator<Item = T>) -> Array
where
  T: Into<JsValue>,
{
  iterable.into_iter().map(|v| v.into()).collect::<Array>()
}

pub struct Renderer {
  canvas: HtmlCanvasElement,
  context: GpuCanvasContext,
  device: GpuDevice,
  queue: GpuQueue,
  pipeline: GpuRenderPipeline,
  depth_texture: GpuTexture,
}

#[derive(Serialize)]
pub struct Color {
  r: f32,
  g: f32,
  b: f32,
  a: f32,
}

impl Renderer {
  pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
    let gpu = window().navigator().gpu();
    let adapter = JsFuture::from(gpu.request_adapter())
      .await?
      .dyn_into::<GpuAdapter>()?;
    let device = JsFuture::from(adapter.request_device())
      .await?
      .dyn_into::<GpuDevice>()?;
    let queue = device.queue();
    let pipeline = {
      let shader =
        device.create_shader_module(&GpuShaderModuleDescriptor::new(include_str!("shader.wgsl")));
      let position_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x3, 0., 0);
      let texcoords_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x2, 0., 1);
      let mut vertex_state = GpuVertexState::new("vs_main", &shader);
      vertex_state.buffers(&iter_to_array(&[
        GpuVertexBufferLayout::new(4. * 3., &iter_to_array(&[position_attribute_description])),
        GpuVertexBufferLayout::new(4. * 3., &iter_to_array(&[texcoords_attribute_description])),
      ]));
      let fragment_state = GpuFragmentState::new(
        "fs_main",
        &shader,
        &iter_to_array(&[GpuColorTargetState::new(GpuTextureFormat::Rgba8unorm)]),
      );
      device.create_render_pipeline(
        &GpuRenderPipelineDescriptor::new(&"auto".into(), &vertex_state)
          .label("Render pipeline")
          .fragment(&fragment_state)
          .primitive(
            &GpuPrimitiveState::new()
              .front_face(GpuFrontFace::Cw)
              .cull_mode(GpuCullMode::Front)
              .topology(GpuPrimitiveTopology::TriangleList),
          )
          .depth_stencil(
            GpuDepthStencilState::new(GpuTextureFormat::Depth24plusStencil8)
              .depth_compare(GpuCompareFunction::Less),
          ),
      )
    };
    let context = canvas
      .get_context("webgpu")?
      .unwrap()
      .dyn_into::<GpuCanvasContext>()?;
    let mut ctx_config = GpuCanvasConfiguration::new(&device, gpu.get_preferred_canvas_format());
    ctx_config.alpha_mode(GpuCanvasAlphaMode::Premultiplied);
    let depth_descriptor = GpuTextureDescriptor::new(
      GpuTextureFormat::Depth24plusStencil8,
      &iter_to_array(&[
        JsValue::from_f64(canvas.width() as f64),
        JsValue::from_f64(canvas.height() as f64),
      ]),
      gpu_texture_usage::RENDER_ATTACHMENT,
    );
    let depth_texture = device.create_texture(&depth_descriptor);
    context.configure(&ctx_config);
    Ok(Self {
      canvas,
      context,
      depth_texture,
      device,
      queue,
      pipeline, // vertex_buffer
    })
  }
  pub fn device(&self) -> &GpuDevice {
    &self.device
  }
  pub fn resize(&self) {
    let (width, height) = get_window_dimension();
    self.canvas.set_width(width);
    self.canvas.set_height(height);
  }
  pub fn render(&self, meshes: Vec<Mesh>, viewport: Viewport) {
    let command_encoder = self.device.create_command_encoder();
    let _ = self.context.get_current_texture().create_view();
    let render_pass_descriptor =
      GpuRenderPassDescriptor::new(&iter_to_array(&[GpuRenderPassColorAttachment::new(
        GpuLoadOp::Clear,
        GpuStoreOp::Store,
        &self.context.get_current_texture().create_view(),
      )
      .clear_value(
        &JsValue::from_serde(&Color {
          r: 0.5,
          g: 0.5,
          b: 0.5,
          a: 0.5,
        })
        .unwrap(),
      )]));
    let pass_encoder = command_encoder.begin_render_pass(&render_pass_descriptor);
    pass_encoder.set_pipeline(&self.pipeline);
    for mesh in meshes {
      // pass_encoder.set_bind_group(0, &obj.model_buffer);
      pass_encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
      pass_encoder.draw(mesh.vertext_count);
    }
    pass_encoder.end();
    self
      .queue
      .submit(&iter_to_array(&[command_encoder.finish()]));
  }
}
