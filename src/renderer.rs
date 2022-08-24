use crate::{
  dom_factory::{create_el, window},
  scene::Mesh,
  viewport::Viewport,
};
use nalgebra::Matrix4;
use std::iter;
use std::rc::Rc;
use wasm_bindgen::JsCast;
use web_sys::HtmlCanvasElement;
use wgpu::{
  util::DeviceExt, Backends, BindGroup, BindGroupEntry, BindGroupLayoutDescriptor,
  BindGroupLayoutEntry, BindingType, BlendComponent, BlendState, Buffer, BufferAddress,
  BufferBindingType, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoderDescriptor,
  Device, DeviceDescriptor, Extent3d, Face, Features, FragmentState, FrontFace, ImageCopyTexture,
  ImageDataLayout, IndexFormat, Instance, Limits, LoadOp, MultisampleState, Operations, Origin3d,
  PipelineLayoutDescriptor, PolygonMode, PowerPreference, PresentMode, PrimitiveState,
  PrimitiveTopology, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
  RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
  ShaderStages, Surface, SurfaceConfiguration, SurfaceError, TextureAspect, TextureDimension,
  TextureFormat, TextureUsages, TextureViewDescriptor, VertexAttribute, VertexBufferLayout,
  VertexFormat, VertexState, VertexStepMode,
};

pub struct Renderer {
  canvas: Rc<HtmlCanvasElement>,
  surface: Surface,
  device: Device,
  queue: Queue,
  config: SurfaceConfiguration,
  render_pipeline: RenderPipeline,
  texture_bind_group: BindGroup,
  viewport_buffer: Buffer,
  model_buffer: Buffer,
  model_view_proj_bind_group: BindGroup,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
  pub position: [f32; 3],
  pub tex_coords: [f32; 2],
}

impl Vertex {
  pub fn description<'a>() -> VertexBufferLayout<'a> {
    VertexBufferLayout {
      array_stride: std::mem::size_of::<Vertex>() as BufferAddress,
      step_mode: VertexStepMode::Vertex,
      attributes: &[
        VertexAttribute {
          offset: 0,
          shader_location: 0,
          format: VertexFormat::Float32x3,
        },
        VertexAttribute {
          offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
          shader_location: 1,
          format: VertexFormat::Float32x2,
        },
      ],
    }
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

impl Renderer {
  pub async fn new() -> Self {
    let el = create_el("canvas");
    let canvas = el.dyn_into::<HtmlCanvasElement>().unwrap();
    let (width, height) = get_window_dimension();

    let instance = Instance::new(Backends::BROWSER_WEBGPU);
    let surface = instance.create_surface_from_canvas(&canvas);
    let adapter = instance
      .request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::default(),
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
      })
      .await
      .expect("Failed to find an appropriate adapter");

    // Create the logical device and command queue
    let (device, queue) = adapter
      .request_device(
        &DeviceDescriptor {
          label: None,
          features: Features::empty(),
          limits: Limits::default(),
        },
        None,
      )
      .await
      .expect("Failed to create device");

    let swapchain_format = surface.get_supported_formats(&adapter)[0];

    let config = SurfaceConfiguration {
      usage: TextureUsages::RENDER_ATTACHMENT,
      format: swapchain_format,
      width,
      height,
      present_mode: PresentMode::Fifo,
    };

    // Load the shaders from disk
    let shader = device.create_shader_module(ShaderModuleDescriptor {
      label: Some("Shader"),
      source: ShaderSource::Wgsl(include_str!("wgsl/shader.wgsl").into()),
    });

    let diffuse_bytes = include_bytes!("img/texture.png");
    let diffuse_image = image::load_from_memory(diffuse_bytes).unwrap();
    let diffuse_rgba = diffuse_image.to_rgba8().into_raw();

    use image::GenericImageView;
    let dimensions = diffuse_image.dimensions();

    let texture_size = Extent3d {
      width: dimensions.0,
      height: dimensions.1,
      depth_or_array_layers: 1,
    };
    let texture = device.create_texture(&wgpu::TextureDescriptor {
      size: texture_size,
      mip_level_count: 1,
      sample_count: 1,
      dimension: TextureDimension::D2,
      format: TextureFormat::Rgba8Unorm,
      usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
      label: Some("texture"),
    });
    queue.write_texture(
      ImageCopyTexture {
        texture: &texture,
        mip_level: 0,
        origin: Origin3d::ZERO,
        aspect: TextureAspect::All,
      },
      &diffuse_rgba,
      ImageDataLayout {
        offset: 0,
        bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
        rows_per_image: std::num::NonZeroU32::new(dimensions.1),
      },
      texture_size,
    );
    let diffuse_texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
      address_mode_u: wgpu::AddressMode::ClampToEdge,
      address_mode_v: wgpu::AddressMode::ClampToEdge,
      address_mode_w: wgpu::AddressMode::ClampToEdge,
      mag_filter: wgpu::FilterMode::Linear,
      min_filter: wgpu::FilterMode::Nearest,
      mipmap_filter: wgpu::FilterMode::Nearest,
      ..Default::default()
    });
    let texture_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
          BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
              multisampled: false,
              view_dimension: wgpu::TextureViewDimension::D2,
              sample_type: wgpu::TextureSampleType::Float { filterable: true },
            },
            count: None,
          },
          BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            // This should match the filterable field of the
            // corresponding Texture entry above.
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
          },
        ],
        label: Some("texture_bind_group_layout"),
      });
    let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &texture_bind_group_layout,
      entries: &[
        BindGroupEntry {
          binding: 0,
          resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
        },
        BindGroupEntry {
          binding: 1,
          resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
        },
      ],
      label: Some("texture_bind_group"),
    });

    let viewport_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("Camera Buffer"),
      contents: bytemuck::cast_slice(Matrix4::<f32>::identity().as_slice()),
      usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });
    let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("Model transformation Buffer"),
      contents: bytemuck::cast_slice(Matrix4::<f32>::identity().as_slice()),
      usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
    });
    let model_view_proj_bind_group_layout =
      device.create_bind_group_layout(&BindGroupLayoutDescriptor {
        entries: &[
          BindGroupLayoutEntry {
            binding: 0,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
              ty: BufferBindingType::Uniform,
              has_dynamic_offset: false,
              min_binding_size: None,
            },
            count: None,
          },
          BindGroupLayoutEntry {
            binding: 1,
            visibility: ShaderStages::VERTEX,
            ty: BindingType::Buffer {
              ty: BufferBindingType::Uniform,
              has_dynamic_offset: false,
              min_binding_size: None,
            },
            count: None,
          },
        ],
        label: Some("model_view_proj_bind_group_layout"),
      });
    let model_view_proj_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      layout: &&model_view_proj_bind_group_layout,
      entries: &[
        BindGroupEntry {
          binding: 0,
          resource: viewport_buffer.as_entire_binding(),
        },
        BindGroupEntry {
          binding: 1,
          resource: model_buffer.as_entire_binding(),
        },
      ],
      label: Some("model_view_proj_bind_group"),
    });
    let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
      label: Some("Render Pipeline Layout"),
      bind_group_layouts: &[
        &model_view_proj_bind_group_layout,
        &texture_bind_group_layout,
      ],
      push_constant_ranges: &[],
    });

    let render_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&pipeline_layout),
      vertex: VertexState {
        module: &shader,
        entry_point: "vs_main",
        buffers: &[Vertex::description()],
      },
      fragment: Some(FragmentState {
        module: &shader,
        entry_point: "fs_main",
        targets: &[Some(ColorTargetState {
          format: swapchain_format,
          blend: Some(BlendState {
            color: BlendComponent::REPLACE,
            alpha: BlendComponent::REPLACE,
          }),
          write_mask: ColorWrites::ALL,
        })],
      }),
      primitive: PrimitiveState {
        topology: PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: FrontFace::Ccw,
        cull_mode: Some(Face::Back),
        polygon_mode: PolygonMode::Fill,
        unclipped_depth: false,
        conservative: false,
      },
      depth_stencil: None,
      multisample: MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
      },
      multiview: None,
    });
    let mut s = Self {
      surface,
      device,
      queue,
      config,
      canvas: Rc::new(canvas),
      render_pipeline,
      texture_bind_group,
      viewport_buffer,
      model_buffer,
      model_view_proj_bind_group,
    };
    log::info!("Renderer created!");
    s.resize();
    s
  }
  pub fn resize(&mut self) {
    let (width, height) = get_window_dimension();
    self.config.width = width;
    self.config.height = height;
    self.canvas.set_width(width);
    self.canvas.set_height(height);
    self.surface.configure(&self.device, &self.config);
  }
  pub fn render(&self, entities: &Vec<Mesh>, viewport: &Viewport) -> Result<(), SurfaceError> {
    let output = self.surface.get_current_texture()?;
    let view = output
      .texture
      .create_view(&TextureViewDescriptor::default());
    let mut encoder = self
      .device
      .create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Render Encoder"),
      });
    self.queue.write_buffer(
      &self.viewport_buffer,
      0,
      bytemuck::cast_slice(&viewport.view_proj().as_slice()),
    );
    {
      let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("Render Pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
          view: &view,
          resolve_target: None,
          ops: Operations {
            load: LoadOp::Clear(Color::BLACK),
            store: true,
          },
        })],

        depth_stencil_attachment: None,
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.model_view_proj_bind_group, &[]);
      render_pass.set_bind_group(1, &self.texture_bind_group, &[]);
      for each in entities {
        self.queue.write_buffer(
          &self.model_buffer,
          0,
          bytemuck::cast_slice(&each.model.to_homogeneous().as_slice()),
        );
        render_pass.set_vertex_buffer(0, each.vertex_buffer.slice(..));
        render_pass.set_index_buffer(each.index_buffer.slice(..), IndexFormat::Uint16);
        render_pass.draw_indexed(0..each.num_indices, 0, 0..1);
      }
    }
    self.queue.submit(iter::once(encoder.finish()));
    output.present();
    Ok(())
  }
  pub fn canvas(&self) -> Rc<HtmlCanvasElement> {
    self.canvas.clone()
  }
  pub fn device(&self) -> &Device {
    &self.device
  }
}
