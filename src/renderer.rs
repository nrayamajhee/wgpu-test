use crate::mesh::Mesh;
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::Array;
use js_sys::Float32Array;
use js_sys::Object;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::gpu_buffer_usage;
use web_sys::{
  gpu_texture_usage, GpuAdapter, GpuAddressMode, GpuBindGroup, GpuBindGroupDescriptor,
  GpuBindGroupEntry, GpuBuffer, GpuBufferBinding, GpuBufferDescriptor, GpuCanvasAlphaMode,
  GpuCanvasConfiguration, GpuCanvasContext, GpuColorTargetState, GpuCompareFunction, GpuCullMode,
  GpuDepthStencilState, GpuDevice, GpuFilterMode, GpuFragmentState, GpuFrontFace,
  GpuImageCopyExternalImage, GpuImageCopyTextureTagged, GpuIndexFormat, GpuLoadOp,
  GpuPrimitiveState, GpuPrimitiveTopology, GpuRenderPassColorAttachment,
  GpuRenderPassDepthStencilAttachment, GpuRenderPassDescriptor, GpuRenderPipeline,
  GpuRenderPipelineDescriptor, GpuSamplerDescriptor, GpuShaderModuleDescriptor, GpuStoreOp,
  GpuTexture, GpuTextureDescriptor, GpuTextureFormat, GpuVertexAttribute, GpuVertexBufferLayout,
  GpuVertexFormat, GpuVertexState, HtmlCanvasElement,
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
  uniform_buffer: GpuBuffer,
  uniform_buffer_bind_group: GpuBindGroup,
  texture_bind_group: GpuBindGroup,
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
    let uniform_buffer = device.create_buffer(&GpuBufferDescriptor::new(
      16. * 4.,
      gpu_buffer_usage::UNIFORM | gpu_buffer_usage::COPY_DST,
    ));
    let mut sampler_desc = GpuSamplerDescriptor::new();
    sampler_desc.address_mode_u(GpuAddressMode::Repeat);
    sampler_desc.address_mode_v(GpuAddressMode::Repeat);
    sampler_desc.mag_filter(GpuFilterMode::Linear);
    let sampler = device.create_sampler_with_descriptor(&sampler_desc);
    let image = gloo_utils::document()
      .create_element("img")?
      .dyn_into::<web_sys::HtmlImageElement>()?;
    image.set_attribute("src", "img/icon.png").unwrap();
    let tex = device.create_texture(&GpuTextureDescriptor::new(
      GpuTextureFormat::Rgba8unormSrgb,
      &iter_to_array([image.width() as i32, image.height() as i32]),
      gpu_texture_usage::TEXTURE_BINDING
        | gpu_texture_usage::COPY_DST
        | gpu_texture_usage::RENDER_ATTACHMENT,
    ));
    let bitmap =
      JsFuture::from(window().create_image_bitmap_with_html_image_element(&image)?).await?;
    let mut source = GpuImageCopyExternalImage::new(&Object::new());
    source.flip_y(true);
    source.source(&Object::from(bitmap));
    device
      .queue()
      .copy_external_image_to_texture_with_u32_sequence(
        &source,
        &GpuImageCopyTextureTagged::new(&tex),
        &JsValue::from_serde(&Rect {
          width: image.width(),
          height: image.height(),
        })
        .unwrap(),
      );
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
              .front_face(GpuFrontFace::Cw)
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
    let uniform_buffer_bind_group = device.create_bind_group(&GpuBindGroupDescriptor::new(
      &iter_to_array(&[JsValue::from(&GpuBindGroupEntry::new(
        0,
        &GpuBufferBinding::new(&uniform_buffer),
      ))]),
      &pipeline.get_bind_group_layout(0),
    ));
    let texture_bind_group = device.create_bind_group(&GpuBindGroupDescriptor::new(
      &iter_to_array(&[
        JsValue::from(&GpuBindGroupEntry::new(0, &sampler)),
        JsValue::from(&GpuBindGroupEntry::new(1, &tex.create_view())),
      ]),
      &pipeline.get_bind_group_layout(1),
    ));
    Ok(Self {
      canvas,
      context,
      device,
      depth_texture,
      uniform_buffer,
      depth_attachment,
      color_attachment,
      pipeline,
      render_pass_descriptor,
      uniform_buffer_bind_group,
      texture_bind_group,
    })
  }
  pub fn canvas(&self) -> &HtmlCanvasElement {
    &self.canvas
  }
  pub fn device(&self) -> &GpuDevice {
    &self.device
  }
  pub fn render(&mut self, mesh: &Mesh, model_view_proj: nalgebra::Matrix4<f32>) {
    let command_encoder = self.device.create_command_encoder();
    self
      .color_attachment
      .view(&self.context.get_current_texture().create_view());
    self
      .render_pass_descriptor
      .color_attachments(&iter_to_array(&[JsValue::from(&self.color_attachment)]));
    self
      .render_pass_descriptor
      .depth_stencil_attachment(&self.depth_attachment);
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
    let queue = self.device.queue();
    pass_encoder.set_vertex_buffer(0, &mesh.vertex_buffer);
    pass_encoder.set_vertex_buffer(1, &mesh.vertex_colors);
    pass_encoder.set_vertex_buffer(2, &mesh.texture_coordinates);
    pass_encoder.set_bind_group(0, &self.uniform_buffer_bind_group);
    pass_encoder.set_bind_group(1, &self.texture_bind_group);
    let model_view_proj = Float32Array::from(model_view_proj.as_slice());
    queue.write_buffer_with_u32_and_buffer_source(&self.uniform_buffer, 0, &model_view_proj);
    pass_encoder.set_index_buffer(&mesh.index_buffer, GpuIndexFormat::Uint16);
    pass_encoder.draw_indexed(mesh.index_count);
    pass_encoder.end();
    let commands = command_encoder.finish();
    queue.submit(&iter_to_array(&[commands]));
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

#[derive(Serialize)]
pub struct Rect {
  width: u32,
  height: u32,
}
