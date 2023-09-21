use crate::iter_to_array;
use crate::mesh::Mesh;
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::Float32Array;
use nalgebra::Matrix4;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_texture_usage, GpuAdapter, GpuCanvasAlphaMode, GpuCanvasConfiguration, GpuCanvasContext,
  GpuColorTargetState, GpuCompareFunction, GpuCullMode, GpuDepthStencilState, GpuDevice,
  GpuFragmentState, GpuFrontFace, GpuIndexFormat, GpuLoadOp, GpuPrimitiveState,
  GpuPrimitiveTopology, GpuRenderPassColorAttachment, GpuRenderPassDepthStencilAttachment,
  GpuRenderPassDescriptor, GpuRenderPipeline, GpuRenderPipelineDescriptor,
  GpuShaderModuleDescriptor, GpuStoreOp, GpuTexture, GpuTextureDescriptor, GpuTextureFormat,
  GpuVertexAttribute, GpuVertexBufferLayout, GpuVertexFormat, GpuVertexState, HtmlCanvasElement,
};

pub struct Renderer {
  canvas: HtmlCanvasElement,
  context: GpuCanvasContext,
  device: GpuDevice,
  pipeline: GpuRenderPipeline,
  depth_texture: GpuTexture,
  color_attachment: GpuRenderPassColorAttachment,
  depth_attachment: GpuRenderPassDepthStencilAttachment,
  render_pass_descriptor: GpuRenderPassDescriptor,
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
    let context = canvas
      .get_context("webgpu")?
      .unwrap()
      .dyn_into::<GpuCanvasContext>()?;
    let (width, height) = get_window_dimension();
    canvas.set_width(width);
    canvas.set_height(height);
    let mut ctx_config = GpuCanvasConfiguration::new(&device, gpu.get_preferred_canvas_format());
    ctx_config.alpha_mode(GpuCanvasAlphaMode::Premultiplied);
    context.configure(&ctx_config);
    let mut color_attachment = GpuRenderPassColorAttachment::new(
      GpuLoadOp::Clear,
      GpuStoreOp::Store,
      &context.get_current_texture().create_view(),
    );
    color_attachment.clear_value(
      &JsValue::from_serde(&Color {
        r: 0.1,
        g: 0.1,
        b: 0.1,
        a: 1.0,
      })
      .unwrap(),
    );
    let depth_descriptor = GpuTextureDescriptor::new(
      GpuTextureFormat::Depth24plusStencil8,
      &iter_to_array(&[
        JsValue::from_f64(width as f64),
        JsValue::from_f64(height as f64),
      ]),
      gpu_texture_usage::RENDER_ATTACHMENT,
    );
    let depth_texture = device.create_texture(&depth_descriptor);
    let mut depth_attachment =
      GpuRenderPassDepthStencilAttachment::new(&depth_texture.create_view());
    depth_attachment
      .depth_clear_value(1.)
      .depth_load_op(GpuLoadOp::Clear)
      .depth_store_op(GpuStoreOp::Store)
      .stencil_clear_value(0)
      .stencil_load_op(GpuLoadOp::Clear)
      .stencil_store_op(GpuStoreOp::Store);
    let mut render_pass_descriptor =
      GpuRenderPassDescriptor::new(&iter_to_array(&[JsValue::from(&color_attachment)]));
    render_pass_descriptor.depth_stencil_attachment(&depth_attachment);
    let pipeline = {
      let shader =
        device.create_shader_module(&GpuShaderModuleDescriptor::new(include_str!("shader.wgsl")));
      let position_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x3, 0., 0);
      let vertex_color_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x3, 0., 1);
      let tex_coords_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x2, 0., 2);
      let mut vertex_state = GpuVertexState::new("vs_main", &shader);
      vertex_state.buffers(&iter_to_array(&[
        GpuVertexBufferLayout::new(4. * 3., &iter_to_array(&[position_attribute_description])),
        GpuVertexBufferLayout::new(
          4. * 3.,
          &iter_to_array(&[vertex_color_attribute_description]),
        ),
        GpuVertexBufferLayout::new(4. * 2., &iter_to_array(&[tex_coords_attribute_description])),
      ]));
      let fragment_state = GpuFragmentState::new(
        "fs_main",
        &shader,
        &iter_to_array(&[GpuColorTargetState::new(gpu.get_preferred_canvas_format())]),
      );
      device.create_render_pipeline(
        &GpuRenderPipelineDescriptor::new(&"auto".into(), &vertex_state)
          .label("Render pipeline")
          .fragment(&fragment_state)
          .primitive(
            &GpuPrimitiveState::new()
              .front_face(GpuFrontFace::Ccw)
              .cull_mode(GpuCullMode::Back)
              .topology(GpuPrimitiveTopology::TriangleList),
          )
          .depth_stencil(
            GpuDepthStencilState::new(GpuTextureFormat::Depth24plusStencil8)
              .depth_compare(GpuCompareFunction::Less)
              .depth_write_enabled(true),
          ),
      )
    };
    Ok(Self {
      canvas,
      context,
      device,
      depth_texture,
      depth_attachment,
      color_attachment,
      pipeline,
      render_pass_descriptor,
    })
  }
  pub fn canvas(&self) -> &HtmlCanvasElement {
    &self.canvas
  }
  pub fn device(&self) -> &GpuDevice {
    &self.device
  }
  pub fn pipeline(&self) -> &GpuRenderPipeline {
    &self.pipeline
  }
  pub fn render(&mut self, meshes: &[Mesh], models: &[Matrix4<f32>], view_proj: Matrix4<f32>) {
    let queue = self.device.queue();
    self
      .color_attachment
      .view(&self.context.get_current_texture().create_view());
    self
      .render_pass_descriptor
      .color_attachments(&iter_to_array(&[JsValue::from(&self.color_attachment)]));
    self
      .render_pass_descriptor
      .depth_stencil_attachment(&self.depth_attachment);
    let command_encoder = self.device.create_command_encoder();
    let pass_encoder = command_encoder.begin_render_pass(&self.render_pass_descriptor);
    pass_encoder.set_pipeline(&self.pipeline);
    pass_encoder.set_viewport(
      0.,
      0.,
      self.canvas.width() as f32,
      self.canvas.height() as f32,
      0.,
      1.,
    );
    pass_encoder.set_scissor_rect(0, 0, self.canvas.width(), self.canvas.height());
    for (mesh, model) in meshes.iter().zip(models.iter()) {
      pass_encoder.set_bind_group(0, &mesh.uniform_bind_group);
      pass_encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
      pass_encoder.set_vertex_buffer(1, &mesh.vertex_colors);
      pass_encoder.set_bind_group(1, &mesh.texture_bind_group);
      pass_encoder.set_vertex_buffer(2, &mesh.texture_coordinates);
      let model_view_proj = view_proj * model;
      let model_view_proj = Float32Array::from(model_view_proj.as_slice());
      queue.write_buffer_with_u32_and_buffer_source(&mesh.uniform_buffer, 0, &model_view_proj);
      pass_encoder.set_index_buffer(&mesh.index_buffer, GpuIndexFormat::Uint16);
      pass_encoder.draw_indexed(mesh.index_count);
    }
    pass_encoder.end();
    queue.submit(&iter_to_array(&[command_encoder.finish()]));
  }
  pub fn resize(&self) {
    let (width, height) = get_window_dimension();
    self.canvas.set_width(width);
    self.canvas.set_height(height);
  }
}

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

#[derive(Serialize)]
pub struct Color {
  r: f32,
  g: f32,
  b: f32,
  a: f32,
}
