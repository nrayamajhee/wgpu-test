use crate::iter_to_array;
use crate::mesh::MaterialType;
use crate::mesh::Mesh;
use crate::viewport::Viewport;
use gloo_utils::format::JsValueSerdeExt;
use gloo_utils::window;
use js_sys::Float32Array;
use js_sys::Uint16Array;
use nalgebra::Isometry3;
use nalgebra::Matrix4;
use nalgebra::Similarity3;
use serde::Serialize;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_buffer_usage, gpu_texture_usage, Blob, Document, GpuAdapter, GpuAddressMode, GpuBuffer,
  GpuBufferDescriptor, GpuCanvasAlphaMode, GpuCanvasConfiguration, GpuCanvasContext,
  GpuColorTargetState, GpuCompareFunction, GpuCullMode, GpuDepthStencilState, GpuDevice,
  GpuFilterMode, GpuFragmentState, GpuFrontFace, GpuIndexFormat, GpuLoadOp, GpuPrimitiveState,
  GpuPrimitiveTopology, GpuRenderPassColorAttachment, GpuRenderPassDepthStencilAttachment,
  GpuRenderPassDescriptor, GpuRenderPipeline, GpuRenderPipelineDescriptor, GpuSampler,
  GpuSamplerDescriptor, GpuShaderModuleDescriptor, GpuStoreOp, GpuTexture, GpuTextureDescriptor,
  GpuTextureDimension, GpuTextureFormat, GpuVertexAttribute, GpuVertexBufferLayout,
  GpuVertexFormat, GpuVertexState, HtmlCanvasElement, ImageBitmap, Response, Window,
};

pub struct Renderer {
  canvas: HtmlCanvasElement,
  context: GpuCanvasContext,
  device: GpuDevice,
  pipeline: GpuRenderPipeline,
  pipeline_cubebox: GpuRenderPipeline,
  depth_texture: GpuTexture,
  color_attachment: GpuRenderPassColorAttachment,
  depth_attachment: GpuRenderPassDepthStencilAttachment,
  render_pass_descriptor: GpuRenderPassDescriptor,
  sampler: GpuSampler,
}

impl Renderer {
  pub async fn new() -> Result<Self, JsValue> {
    let canvas = window()
      .document()
      .unwrap()
      .create_element("canvas")?
      .dyn_into::<HtmlCanvasElement>()?;
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
    let (pipeline, pipeline_cubebox) = {
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
      let pipeline = device.create_render_pipeline(
        &GpuRenderPipelineDescriptor::new(&"auto".into(), &vertex_state)
          .label("Defualt Render pipeline")
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
      );
      let cubemap_shader = device.create_shader_module(&GpuShaderModuleDescriptor::new(
        include_str!("shader_cube.wgsl"),
      ));
      let position_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x3, 0., 0);
      let tex_coords_attribute_description =
        GpuVertexAttribute::new(GpuVertexFormat::Float32x2, 0., 1);
      let mut vertex_state = GpuVertexState::new("vs_main", &cubemap_shader);
      vertex_state.buffers(&iter_to_array(&[
        GpuVertexBufferLayout::new(4. * 3., &iter_to_array(&[position_attribute_description])),
        GpuVertexBufferLayout::new(4. * 2., &iter_to_array(&[tex_coords_attribute_description])),
      ]));
      let fragment_state = GpuFragmentState::new(
        "fs_main",
        &cubemap_shader,
        &iter_to_array(&[GpuColorTargetState::new(gpu.get_preferred_canvas_format())]),
      );
      let pipeline_cubemap = device.create_render_pipeline(
        &GpuRenderPipelineDescriptor::new(&"auto".into(), &vertex_state)
          .label("Cubemap Render pipeline")
          .fragment(&fragment_state)
          .primitive(
            &GpuPrimitiveState::new()
              .front_face(GpuFrontFace::Ccw)
              .cull_mode(GpuCullMode::Front)
              .topology(GpuPrimitiveTopology::TriangleList),
          )
          .depth_stencil(
            GpuDepthStencilState::new(GpuTextureFormat::Depth24plusStencil8)
              .depth_compare(GpuCompareFunction::Less)
              .depth_write_enabled(true),
          ),
      );
      (pipeline, pipeline_cubemap)
    };
    let mut sampler_desc = GpuSamplerDescriptor::new();
    sampler_desc.address_mode_u(GpuAddressMode::Repeat);
    sampler_desc.address_mode_v(GpuAddressMode::Repeat);
    sampler_desc.mag_filter(GpuFilterMode::Linear);
    let sampler = device.create_sampler_with_descriptor(&sampler_desc);
    Ok(Self {
      canvas,
      context,
      device,
      depth_texture,
      depth_attachment,
      color_attachment,
      pipeline,
      pipeline_cubebox,
      render_pass_descriptor,
      sampler,
    })
  }
  pub fn texture_sampler(&self) -> &GpuSampler {
    &self.sampler
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
  pub fn pipeline_cubebox(&self) -> &GpuRenderPipeline {
    &self.pipeline_cubebox
  }
  pub fn render(&mut self, meshes: &[Mesh], models: &[Similarity3<f32>], viewport: &Viewport) {
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
      if mesh.material_type == MaterialType::CubeMap {
        pass_encoder.set_pipeline(&self.pipeline_cubebox);
      } else {
        pass_encoder.set_pipeline(&self.pipeline);
      }
      pass_encoder.set_vertex_buffer(0, &mesh.vertex_buffer);

      match mesh.material_type {
        MaterialType::CubeMap => {
          pass_encoder.set_vertex_buffer(1, &mesh.texture_coordinates);
        }
        _ => {
          pass_encoder.set_vertex_buffer(1, &mesh.vertex_colors);
          pass_encoder.set_vertex_buffer(2, &mesh.texture_coordinates);
        }
      }

      pass_encoder.set_bind_group(0, &mesh.uniform_bind_group);
      pass_encoder.set_bind_group(1, &mesh.texture_bind_group);

      if matches!(mesh.material_type, MaterialType::CubeMap) {
        let mvp = viewport.view_cube() * model.to_homogeneous();
        let uniforms = Float32Array::from(&mvp.as_slice()[..]);
        queue.write_buffer_with_u32_and_buffer_source(&mesh.uniform_buffer, 0, &uniforms);
      } else {
        let mvp = viewport.view_proj() * model.to_homogeneous();
        let Color { r, g, b, a } = mesh.color;
        let mut uniforms: Vec<f32> = mvp.into_iter().map(|f| *f).collect();
        uniforms.push(r);
        uniforms.push(g);
        uniforms.push(b);
        uniforms.push(a);
        uniforms.push(mesh.material_type as u32 as f32);
        let uniforms = Float32Array::from(&uniforms[..]);
        queue.write_buffer_with_u32_and_buffer_source(&mesh.uniform_buffer, 0, &uniforms);
      }
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
  pub fn create_buffer(&self, data: &[f32]) -> GpuBuffer {
    let byte_len = data.len() * 4;
    let size = byte_len + 3 & !3;
    let buffer = self.device.create_buffer(
      &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
    );
    let write_array = Float32Array::new(&buffer.get_mapped_range());
    write_array.set(&Float32Array::from(&data[..]), 0);
    buffer.unmap();
    buffer
  }
  pub fn create_index_buffer(&self, data: &[u16]) -> GpuBuffer {
    let size = data.len() * 2;
    let size = size + 3 & !3;
    let buffer = self.device.create_buffer(
      &GpuBufferDescriptor::new(
        size as f64,
        gpu_buffer_usage::INDEX | gpu_buffer_usage::COPY_DST,
      )
      .mapped_at_creation(true),
    );
    let write_array = Uint16Array::new(&buffer.get_mapped_range());
    write_array.set(&Uint16Array::from(&data[..]), 0);
    buffer.unmap();
    buffer
  }
  pub fn create_texture(&self, rect: &Rect, num_images: u32) -> GpuTexture {
    let mut desc = GpuTextureDescriptor::new(
      GpuTextureFormat::Rgba8unorm,
      &iter_to_array([rect.width as u32, rect.height as u32, num_images]),
      gpu_texture_usage::TEXTURE_BINDING
        | gpu_texture_usage::COPY_DST
        | gpu_texture_usage::RENDER_ATTACHMENT,
    );
    if num_images == 6 {
      desc.dimension(GpuTextureDimension::N2d);
    }
    self.device.create_texture(&desc)
  }
  pub async fn create_bitmap(src: &str) -> Result<(ImageBitmap, Rect), JsValue> {
    let res = JsFuture::from(window().fetch_with_str(src))
      .await?
      .dyn_into::<Response>()?;
    let blob = JsFuture::from(res.blob()?).await?.dyn_into::<Blob>()?;
    let bitmap = JsFuture::from(window().create_image_bitmap_with_blob(&blob)?).await?;
    let image = bitmap.dyn_into::<ImageBitmap>()?;
    let (width, height) = (image.width(), image.height());
    Ok((image, Rect { width, height }))
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

#[derive(Serialize, Clone, Copy, Debug)]
pub struct Color {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  pub a: f32,
}

impl Color {
  pub fn rgb(r: f32, g: f32, b: f32) -> Self {
    Self { r, g, b, a: 1. }
  }
}

#[derive(Serialize)]
pub struct Rect {
  pub width: u32,
  pub height: u32,
}
