use crate::{
  dom_factory::{create_el, window},
  mesh::{Geometry, Mesh, Vertex},
};
use gloo_utils::format::JsValueSerdeExt;
use js_sys::{Array, Float32Array, WebAssembly};
use nalgebra::Similarity3;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
  gpu_buffer_usage, gpu_shader_stage, gpu_texture_usage, GpuAdapter, GpuBindGroupDescriptor,
  GpuBindGroupLayoutDescriptor, GpuBuffer, GpuBufferDescriptor, GpuCanvasAlphaMode,
  GpuCanvasConfiguration, GpuCanvasContext, GpuColorTargetState, GpuCompareFunction, GpuCullMode,
  GpuDepthStencilState, GpuDevice, GpuFragmentState, GpuFrontFace, GpuIndexFormat, GpuLoadOp,
  GpuPipelineLayoutDescriptor, GpuPrimitiveState, GpuPrimitiveTopology, GpuQueue,
  GpuRenderPassColorAttachment, GpuRenderPassDepthStencilAttachment, GpuRenderPassDescriptor,
  GpuRenderPipeline, GpuRenderPipelineDescriptor, GpuShaderModuleDescriptor, GpuStoreOp,
  GpuTextureDescriptor, GpuTextureDimension, GpuTextureFormat, GpuVertexState, HtmlCanvasElement,
};

fn iter_to_array<T>(iterable: impl IntoIterator<Item = T>) -> Array
where
  T: Into<JsValue>,
{
  iterable.into_iter().map(|v| v.into()).collect::<Array>()
}

pub struct Canvas {
  element: HtmlCanvasElement,
  context: GpuCanvasContext,
}

impl Canvas {
  pub fn new() -> Self {
    let el = create_el("canvas");
    let element = el.dyn_into::<HtmlCanvasElement>().unwrap();
    let context = element
      .get_context("webgpu")
      .unwrap()
      .unwrap()
      .dyn_into::<GpuCanvasContext>()
      .unwrap();
    Self { element, context }
  }
  pub fn configure(&self, device: &GpuDevice) {
    let canvas_config = {
      let mut canvas_config = GpuCanvasConfiguration::new(&device, GpuTextureFormat::Rgba8unorm);
      canvas_config.usage(gpu_texture_usage::RENDER_ATTACHMENT | gpu_texture_usage::COPY_SRC);
      canvas_config.alpha_mode(GpuCanvasAlphaMode::Opaque);
      canvas_config
    };
    self.context.configure(&canvas_config);
  }
  pub fn resize(&self) {
    let (width, height) = get_window_dimension();
    self.element.set_width(width);
    self.element.set_height(height);
  }
  pub fn size(&self) -> [u32; 2] {
    [self.element.width(), self.element.height()]
  }
  pub fn context(&self) -> &GpuCanvasContext {
    &self.context
  }
}

pub struct Attachments {
  color: GpuRenderPassColorAttachment,
  depth: GpuRenderPassDepthStencilAttachment,
}

impl Attachments {
  pub fn new(device: &GpuDevice, canvas: &Canvas) -> Self {
    let color_texture = canvas.context().get_current_texture();
    let color_texture_view = color_texture.create_view();
    let mut color =
      GpuRenderPassColorAttachment::new(GpuLoadOp::Clear, GpuStoreOp::Store, &color_texture_view);
    color.clear_value(&JsValue::from_serde("{r: 0, g: 0, b: 0, a: 1}").unwrap());
    let depth_texture = device.create_texture(
      &GpuTextureDescriptor::new(
        GpuTextureFormat::Depth24plusStencil8,
        &iter_to_array(vec![&canvas.size()[..], &[1]].concat()),
        gpu_texture_usage::RENDER_ATTACHMENT | gpu_texture_usage::COPY_SRC,
      )
      .dimension(GpuTextureDimension::N2d),
    );
    let depth_texture_view = depth_texture.create_view();
    let mut depth = GpuRenderPassDepthStencilAttachment::new(&depth_texture_view);
    depth
      .depth_clear_value(1.)
      .depth_load_op(GpuLoadOp::Load)
      .depth_store_op(GpuStoreOp::Store)
      .stencil_clear_value(0)
      .stencil_load_op(GpuLoadOp::Load)
      .stencil_store_op(GpuStoreOp::Store);
    Self { color, depth }
  }
}

pub struct Renderer {
  canvas: Canvas,
  device: GpuDevice,
  attachments: Attachments,
  render_pipeline: GpuRenderPipeline,
  // surface: Surface,
  // queue: Queue,
  // config: SurfaceConfiguration,
  // texture_bind_group: BindGroup,
  // viewport_buffer: Buffer,
  // model_buffer: Buffer,
  // model_view_proj_bind_group: BindGroup,
}

// impl Vertex {
//   pub fn description<'a>() -> VertexBufferLayout<'a> {
//     VertexBufferLayout {
//       array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
//       step_mode: VertexStepMode::Vertex,
//       attributes: &[
//         VertexAttribute {
//           offset: 0,
//           shader_location: 0,
//           format: VertexFormat::Float32x3,
//         },
//         VertexAttribute {
//           offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
//           shader_location: 1,
//           format: VertexFormat::Float32x2,
//         },
//       ],
//     }
//   }
// }

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

impl Renderer {
  pub async fn new() -> Self {
    let device = JsFuture::from(
      JsFuture::from(window().navigator().gpu().request_adapter())
        .await
        .unwrap()
        .dyn_into::<GpuAdapter>()
        .unwrap()
        .request_device(),
    )
    .await
    .unwrap()
    .dyn_into::<GpuDevice>()
    .unwrap();

    let canvas = Canvas::new();
    canvas.configure(&device);

    let shader = device.create_shader_module(&GpuShaderModuleDescriptor::new(include_str!(
      "wgsl/shader.wgsl"
    )));
    let uniform_bind_group_layout =
      device.create_bind_group_layout(&GpuBindGroupLayoutDescriptor::new(
        &JsValue::from_serde(&format!(
          "[
            binding: 0,
            visibility: {}, 
            buffer: {{}}
        ]",
          gpu_shader_stage::VERTEX
        ))
        .unwrap(),
      ));
    let uniform_bind_group = device.create_bind_group(&GpuBindGroupDescriptor::new(
      &JsValue::from_serde(
        "[
            binding: 0,
        ]",
      )
      .unwrap(),
      &uniform_bind_group_layout,
    ));
    let layout =
      device.create_pipeline_layout(&GpuPipelineLayoutDescriptor::new(&iter_to_array(&[
        uniform_bind_group_layout,
      ])));
    let states = [GpuColorTargetState::new(GpuTextureFormat::Rgba8unorm)];
    let render_pipeline = device.create_render_pipeline(
      &GpuRenderPipelineDescriptor::new(&layout, &GpuVertexState::new("vs_main", &shader))
        .label("Render pipeline")
        .fragment(&GpuFragmentState::new(
          "fs_main",
          &shader,
          &iter_to_array(&states),
        ))
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
    );
    let attachments = Attachments::new(&device, &canvas);
    // .clear_value(1.);
    //
    //
    //   // Load the shaders from disk
    //
    //   let diffuse_bytes = include_bytes!("img/texture.png");
    //   let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
    //   let diffuse_rgba = diffuse_image.to_rgba8().into_raw();
    //
    //   use image::GenericImageView;
    //   let dimensions = diffuse_image.dimensions();
    //
    //   let texture_size = Extent3d {
    //     width: dimensions.0,
    //     height: dimensions.1,
    //     depth_or_array_layers: 1,
    //   };
    //   let texture = device.create_texture(&wgpu::TextureDescriptor {
    //     size: texture_size,
    //     mip_level_count: 1,
    //     sample_count: 1,
    //     dimension: TextureDimension::D2,
    //     format: TextureFormat::Rgba8Unorm,render
    //     usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
    //     label: Some("texture"),
    //   });
    //   queue.write_texture(
    //     ImageCopyTexture {
    //       texture: &texture,
    //       mip_level: 0,
    //       origin: Origin3d::ZERO,
    //       aspect: TextureAspect::All,
    //     },
    //     &diffuse_rgba,
    //     ImageDataLayout {
    //       offset: 0,
    //       bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
    //       rows_per_image: std::num::NonZeroU32::new(dimensions.1),
    //     },
    //     texture_size,
    //   );
    //   let diffuse_texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    //   let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
    //     address_mode_u: wgpu::AddressMode::ClampToEdge,
    //     address_mode_v: wgpu::AddressMode::ClampToEdge,
    //     address_mode_w: wgpu::AddressMode::ClampToEdge,
    //     mag_filter: wgpu::FilterMode::Linear,
    //     min_filter: wgpu::FilterMode::Nearest,
    //     mipmap_filter: wgpu::FilterMode::Nearest,
    //     ..Default::default()
    //   });
    //   let texture_bind_group_layout =
    //     device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
    //       entries: &[
    //         BindGroupLayoutEntry {
    //           binding: 0,
    //           visibility: wgpu::ShaderStages::FRAGMENT,
    //           ty: wgpu::BindingType::Texture {
    //             multisampled: false,
    //             view_dimension: wgpu::TextureViewDimension::D2,
    //             sample_type: wgpu::TextureSampleType::Float { filterable: true },
    //           },
    //           count: None,
    //         },
    //         BindGroupLayoutEntry {
    //           binding: 1,
    //           visibility: wgpu::ShaderStages::FRAGMENT,
    //           // This should match the filterable field of the
    //           // corresponding Texture entry above.
    //           ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
    //           count: None,
    //         },
    //       ],
    //       label: Some("texture_bind_group_layout"),
    //     });
    //   let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    //     layout: &texture_bind_group_layout,
    //     entries: &[
    //       BindGroupEntry {
    //         binding: 0,
    //         resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
    //       },
    //       BindGroupEntry {
    //         binding: 1,
    //         resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
    //       },
    //     ],
    //     label: Some("texture_bind_group"),
    //   });
    //
    //   let viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //     label: Some("Camera Buffer"),
    //     contents: bytemuck::cast_slice(Matrix4::<f32>::identity().as_slice()),
    //     usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    //   });
    //   let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
    //     label: Some("Model transformation Buffer"),
    //     contents: bytemuck::cast_slice(Matrix4::<f32>::identity().as_slice()),
    //     usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    //   });
    //   let model_view_proj_bind_group_layout =
    //     device.create_bind_group_layout(&BindGroupLayoutDescriptor {
    //       entries: &[
    //         BindGroupLayoutEntry {
    //           binding: 0,
    //           visibility: ShaderStages::VERTEX,
    //           ty: BindingType::Buffer {
    //             ty: BufferBindingType::Uniform,
    //             has_dynamic_offset: false,
    //             min_binding_size: None,
    //           },
    //           count: None,
    //         },
    //         BindGroupLayoutEntry {
    //           binding: 1,
    //           visibility: ShaderStages::VERTEX,
    //           ty: BindingType::Buffer {
    //             ty: BufferBindingType::Uniform,
    //             has_dynamic_offset: false,
    //             min_binding_size: None,
    //           },
    //           count: None,
    //         },
    //       ],
    //       label: Some("model_view_proj_bind_group_layout"),
    //     });
    //   let model_view_proj_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
    //     layout: &&model_view_proj_bind_group_layout,
    //     entries: &[
    //       BindGroupEntry {
    //         binding: 0,
    //         resource: viewport_buffer.as_entire_binding(),
    //       },
    //       BindGroupEntry {
    //         binding: 1,
    //         resource: model_buffer.as_entire_binding(),
    //       },
    //     ],
    //     label: Some("model_view_proj_bind_group"),
    //   });
    //   let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
    //     label: Some("Render Pipeline Layout"),
    //     bind_group_layouts: &[
    //       &model_view_proj_bind_group_layoutiew_proj_bind_group_layout,
    //       &texture_bind_group_layout,
    //     ],
    //     push_constant_ranges: &[],
    //   });
    //
    // .label(Some("Render Pipeline"))
    // .layout(Some(&pipeline_layout))
    // .vertex(VertexState {
    //   module: &shader,
    //   entry_point: "vs_main",
    //   buffers: &[Vertex::description()],
    // });
    //   fragment: Some(FragmentState {
    //     module: &shader,
    //     entry_point: "fs_main",
    //     targets: &[Some(ColorTargetState {
    //       format: swapchain_format,
    //       blend: Some(BlendState {
    //         color: BlendComponent::REPLACE,
    //         alpha: BlendComponent::REPLACE,
    //       }),
    //       write_mask: ColorWrites::ALL,
    //     })],
    //   }),
    //   primitive: PrimitiveState {
    //     topology: PrimitiveTopology::TriangleList,
    //     strip_index_format: None,
    //     front_face: FrontFace::Ccw,
    //     cull_mode: Some(Face::Back),
    //     polygon_mode: PolygonMode::Fill,
    //     unclipped_depth: false,
    //     conservative: false,
    //   },
    //   depth_stencil: None,
    //   multisample: MultisampleState {
    //     count: 1,
    //     mask: !0,
    //     alpha_to_coverage_enabled: false,
    //   },
    //   multiview: None,
    // });
    let mut s = Self {
      canvas,
      //     surface,
      device,
      //     queue,
      //     config,
      render_pipeline,
      attachments,
      //     texture_bind_group,
      //     viewport_buffer,
      //     model_buffer,
      //     model_view_proj_bind_group,
    };
    log::info!("Renderer created!");
    // s.resize();
    s
  }
  pub fn resize(&mut self) {
    // self.config.width = width;
    // self.config.height = height;
    self.canvas.resize();
    // self.surface.configure(&self.device, &self.config);
  }
  pub fn render(&self, entities: &Vec<Mesh>) {
    // pub fn render(&self, entities: &Vec<Mesh>, viewport: &Viewport) -> Result<(), SurfaceError> {
    let command_encoder = self.device.create_command_encoder();
    let [width, height] = self.canvas.size();
    let pass = command_encoder.begin_render_pass(
      &GpuRenderPassDescriptor::new(&self.attachments.color)
        .depth_stencil_attachment(&self.attachments.depth),
    );
    pass.set_pipeline(&self.render_pipeline);
    pass.set_viewport(0., 0., width as f32, height as f32, 0., 1.);
    pass.set_scissor_rect(0, 0, width, height);
    for entity in entities {
      pass.set_vertex_buffer(0, &entity.vertex_buffer);
      // pass.set_vertex_buffer(1, colorBuffer);
      pass.set_index_buffer(&entity.index_buffer, GpuIndexFormat::Uint16);
      pass.draw_indexed(3);
    }
    pass.end();
    self
      .device
      .queue()
      .submit(&iter_to_array(&[command_encoder.finish()]));
  }
  //   let output = self.surface.get_current_texture()?;
  //   let view = output
  //     .texture
  //     .create_view(&TextureViewDescriptor::default());
  //   let mut encoder = self
  //     .device
  //     .create_command_encoder(&CommandEncoderDescriptor {
  //       label: Some("Render Encoder"),
  //     });
  //   self.queue.write_buffer(
  //     &self.viewport_buffer,
  //     0,
  //     bytemuck::cast_slice(&viewport.view_proj().as_slice()),
  //   );
  //   {
  //     let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
  //       label: Some("Render Pass"),
  //       color_attachments: &[Some(RenderPassColorAttachment {
  //         view: &view,
  //         resolve_target: None,
  //         ops: Operations {
  //           load: LoadOp::Clear(Color::BLACK),
  //           store: true,
  //         },
  //       })],
  //
  //       depth_stencil_attachment: None,
  //     });
  //     render_pass.set_pipeline(&self.render_pipeline);
  //     render_pass.set_bind_group(0, &self.model_view_proj_bind_group, &[]);
  //     render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
  //     for each in entities {
  //       self.queue.write_buffer(
  //         &self.model_buffer,
  //         0,
  //         bytemuck::cast_slice(&each.model.to_homogeneous().as_slice()),
  //       );
  //       render_pass.set_vertex_buffer(0, each.vertex_buffer.slice(..));
  //       render_pass.set_index_buffer(each.index_buffer.slice(..), IndexFormat::Uint16);
  //       render_pass.draw_indexed(0..each.num_indices, 0, 0..1);
  //     }
  //   }
  //   self.queue.submit(iter::once(encoder.finish()));
  //   output.present();
  // Ok(())
  // }
  pub fn device(&self) -> &GpuDevice {
    &self.device
  }
  pub fn mesh(&self, name: &str, geometry: Geometry) -> Mesh {
    let size = geometry.vertices.len() * 3 * 4;
    let vertex_buffer = self.device.create_buffer(
      &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::VERTEX).mapped_at_creation(true),
    );
    let memory_buffer = wasm_bindgen::memory()
      .dyn_into::<WebAssembly::Memory>()
      .unwrap()
      .buffer();
    let array =
      Float32Array::new_with_byte_offset(&memory_buffer, geometry.vertices.as_ptr() as u32);
    let write_array = Float32Array::new(&vertex_buffer.get_mapped_range());
    write_array.set(&array, 0);
    vertex_buffer.unmap();
    let size = geometry.indices.len() * 3 * 4;
    let index_buffer = self.device.create_buffer(
      &GpuBufferDescriptor::new(size as f64, gpu_buffer_usage::INDEX).mapped_at_creation(true),
    );
    let array =
      Float32Array::new_with_byte_offset(&memory_buffer, geometry.indices.as_ptr() as u32);
    let write_array = Float32Array::new(&index_buffer.get_mapped_range());
    write_array.set(&array, 0);
    index_buffer.unmap();
    let model = Similarity3::identity();
    Mesh {
      vertex_buffer,
      index_buffer,
      num_indices: geometry.indices.len() as u32,
      model,
    }
  }
  pub fn create_buffer(device: &GpuDevice, array: &[f32], size: usize, usage: u32) -> GpuBuffer {
    let size = array.len() * size * 4;
    let buffer =
      device.create_buffer(&GpuBufferDescriptor::new(size as f64, usage).mapped_at_creation(true));
    let memory_buffer = wasm_bindgen::memory()
      .dyn_into::<WebAssembly::Memory>()
      .unwrap()
      .buffer();
    let array = Float32Array::new_with_byte_offset(&memory_buffer, array.as_ptr() as u32);
    let write_array = Float32Array::new(&buffer.get_mapped_range());
    write_array.set(&array, 0);
    buffer.unmap();
    buffer
  }
}
