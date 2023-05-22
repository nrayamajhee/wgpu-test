use crate::mesh::Mesh;
use crate::viewport::Viewport;
use gloo_console::log;
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::Array;
use js_sys::Float32Array;
use js_sys::Object;
use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::to_value;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::gpu_buffer_usage;
use web_sys::GpuTexture;
use web_sys::{
  gpu_shader_stage, gpu_texture_usage, GpuAdapter, GpuBindGroup, GpuBindGroupDescriptor,
  GpuBindGroupEntry, GpuBindGroupLayout, GpuBindGroupLayoutDescriptor, GpuBindGroupLayoutEntry,
  GpuBuffer, GpuBufferBinding, GpuBufferBindingLayout, GpuBufferDescriptor, GpuCanvasAlphaMode,
  GpuCanvasConfiguration, GpuCanvasContext, GpuColorTargetState, GpuCompareFunction, GpuCullMode,
  GpuDepthStencilState, GpuDevice, GpuFragmentState, GpuFrontFace, GpuIndexFormat, GpuLoadOp,
  GpuPipelineLayout, GpuPipelineLayoutDescriptor, GpuPrimitiveState, GpuPrimitiveTopology,
  GpuQueue, GpuRenderPassColorAttachment, GpuRenderPassDepthStencilAttachment,
  GpuRenderPassDescriptor, GpuRenderPassEncoder, GpuRenderPipeline, GpuRenderPipelineDescriptor,
  GpuShaderModuleDescriptor, GpuStoreOp, GpuTextureDescriptor, GpuTextureFormat,
  GpuVertexAttribute, GpuVertexBufferLayout, GpuVertexFormat, GpuVertexState, HtmlCanvasElement,
};

pub struct Renderer {
  canvas: HtmlCanvasElement,
  context: GpuCanvasContext,
  device: GpuDevice,
  queue: GpuQueue,
  pipeline: GpuRenderPipeline,
  depth_texture: GpuTexture,
  color_attachment: GpuRenderPassColorAttachment,
  depth_attachment: GpuRenderPassDepthStencilAttachment,
  render_pass_descriptor: GpuRenderPassDescriptor,
  uniform_buffer: GpuBuffer,
  uniform_buffer_bind_group: GpuBindGroup,
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
    let context = canvas
      .get_context("webgpu")?
      .unwrap()
      .dyn_into::<GpuCanvasContext>()?;
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
      GpuTextureFormat::Depth24plus,
      &iter_to_array(&[
        JsValue::from_f64(canvas.width() as f64),
        JsValue::from_f64(canvas.height() as f64),
      ]),
      gpu_texture_usage::RENDER_ATTACHMENT,
    );
    let depth_texture = device.create_texture(&depth_descriptor);
    let mut depth_attachment =
      GpuRenderPassDepthStencilAttachment::new(&depth_texture.create_view());
    depth_attachment
      .depth_load_op(GpuLoadOp::Load)
      .depth_store_op(GpuStoreOp::Store)
      .depth_clear_value(1.0);
    let mut render_pass_descriptor =
      GpuRenderPassDescriptor::new(&iter_to_array(&[JsValue::from(&color_attachment)]));
    render_pass_descriptor.depth_stencil_attachment(&depth_attachment);
    let uniform_buffer = device.create_buffer(&GpuBufferDescriptor::new(
      16. * 4.,
      gpu_buffer_usage::UNIFORM | gpu_buffer_usage::COPY_DST,
    ));
    let mut layout_entry = GpuBindGroupLayoutEntry::new(0, gpu_shader_stage::VERTEX);
    layout_entry.buffer(&GpuBufferBindingLayout::new());
    let entries: Array = [layout_entry].iter().collect();
    let uniform_bind_group_layout =
      device.create_bind_group_layout(&GpuBindGroupLayoutDescriptor::new(&entries));
    let uniform_buffer_bind_group = device.create_bind_group(&GpuBindGroupDescriptor::new(
      &iter_to_array(&[JsValue::from(&GpuBindGroupEntry::new(
        0,
        &GpuBufferBinding::new(&uniform_buffer),
      ))]),
      &uniform_bind_group_layout,
    ));
    let pipeline = {
      let shader =
        device.create_shader_module(&GpuShaderModuleDescriptor::new(include_str!("shader.wgsl")));
      let position_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x3, 0., 0);
      let vertexcolor_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x3, 0., 1);
      // let texcoords_attribute_description =
      // GpuVertexAttribute::new(GpuVertexFormat::Float32x2, 0., 1);
      let mut vertex_state = GpuVertexState::new("vs_main", &shader);
      vertex_state.buffers(&iter_to_array(&[
        GpuVertexBufferLayout::new(4. * 3., &iter_to_array(&[position_attribute_description])),
        GpuVertexBufferLayout::new(
          4. * 3.,
          &iter_to_array(&[vertexcolor_attribute_description]),
        ),
        // GpuVertexBufferLayout::new(4. * 3., &iter_to_array(&[texcoords_attribute_description])),
      ]));
      let fragment_state = GpuFragmentState::new(
        "fs_main",
        &shader,
        &iter_to_array(&[GpuColorTargetState::new(gpu.get_preferred_canvas_format())]),
      );
      let pipeline_layout_description =
        GpuPipelineLayoutDescriptor::new(&iter_to_array(&[uniform_bind_group_layout]));
      let layout = device.create_pipeline_layout(&pipeline_layout_description);
      device.create_render_pipeline(
        &GpuRenderPipelineDescriptor::new(&layout, &vertex_state)
          .label("Render pipeline")
          .fragment(&fragment_state)
          .primitive(
            &GpuPrimitiveState::new()
              .front_face(GpuFrontFace::Cw)
              .cull_mode(GpuCullMode::Front)
              .topology(GpuPrimitiveTopology::TriangleList),
          )
          .depth_stencil(
            GpuDepthStencilState::new(GpuTextureFormat::Depth24plus)
              .depth_compare(GpuCompareFunction::Less)
              .depth_write_enabled(true),
          ),
      )
    };
    Ok(Self {
      canvas,
      context,
      device,
      queue,
      depth_texture,
      color_attachment,
      depth_attachment,
      render_pass_descriptor,
      pipeline,
      uniform_buffer,
      uniform_buffer_bind_group,
    })
  }
  pub fn canvas(&self) -> &HtmlCanvasElement {
    &self.canvas
  }
  pub fn device(&self) -> &GpuDevice {
    &self.device
  }
  pub fn render(&mut self, meshes: &[Mesh], viewport: &Viewport) {
    let command_encoder = self.device.create_command_encoder();
    self
      .color_attachment
      .view(&self.context.get_current_texture().create_view());
    self
      .render_pass_descriptor
      .color_attachments(&iter_to_array(&[JsValue::from(&self.color_attachment)]));
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
    for mesh in meshes {
      pass_encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
      pass_encoder.set_vertex_buffer(1, &mesh.vertex_colors);
      pass_encoder.set_bind_group(0, &self.uniform_buffer_bind_group);
      let view_proj = Float32Array::from(viewport.view_proj().as_slice());
      self.queue.write_buffer_with_u32_and_buffer_source_and_u32(
        &self.uniform_buffer,
        0,
        &view_proj,
        16,
      );
      pass_encoder.set_index_buffer(&mesh.index_buffer, GpuIndexFormat::Uint16);
      pass_encoder.draw_indexed(mesh.vertext_count);
    }
    pass_encoder.end();
    let commands = command_encoder.finish();
    self.queue.submit(&iter_to_array(&[commands]));
  }
}

pub fn iter_to_array<T>(iterable: impl IntoIterator<Item = T>) -> Array
where
  T: Into<JsValue>,
{
  iterable.into_iter().map(|v| v.into()).collect::<Array>()
}

#[derive(Serialize)]
pub struct Color {
  r: f32,
  g: f32,
  b: f32,
  a: f32,
}
