use std::any;

use netcanv_renderer::paws::Ui;
use wgpu::util::DeviceExt;
pub use winit;

mod error;
mod rendering;

use anyhow::Context;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use rendering::*;

pub struct WgpuBackend {
   window: Window,

   instance: wgpu::Instance,
   surface: wgpu::Surface,
   adapter: wgpu::Adapter,
   device: wgpu::Device,
   queue: wgpu::Queue,

   vertex_buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   render_pipeline: wgpu::RenderPipeline,
}

impl WgpuBackend {
   pub async fn new(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
   ) -> anyhow::Result<Self> {
      let window = window_builder.build(event_loop).context("Failed to create window")?;
      let instance = wgpu::Instance::default();

      let surface = unsafe { instance.create_surface(&window) }
         .context("Failed to create surface from window")?;
      let adapter = instance
         .request_adapter(&wgpu::RequestAdapterOptionsBase {
            power_preference: wgpu::PowerPreference::LowPower,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
         })
         .await
         .context(
            "Failed to find a graphics adapter. Please make sure your drivers are up to date",
         )?;

      let swapchain_capabilities = surface.get_capabilities(&adapter);
      let swapchain_format = swapchain_capabilities.formats[0];

      let (device, queue) = adapter.request_device(
         &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
         },
         None,
      ).await.context("Failed to acquire graphics device. Try updating your graphics drivers. If that doesn't work, your hardware may be too old to run NetCanv.")?;

      let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
         label: Some("Immediate Geometry Vertex Buffer"),
         contents: bytemuck::cast_slice(&[-0.5_f32, -0.5, 0.5, -0.5, 0.0, 0.5]),
         usage: wgpu::BufferUsages::VERTEX,
      });

      let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
         label: Some("Immediate Geometry Bind Group Layout"),
         entries: &[],
      });
      let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
         label: Some("Immediate Geometry Pipeline Layout"),
         bind_group_layouts: &[&bind_group_layout],
         push_constant_ranges: &[],
      });
      let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some("Immediate Geometry Bind Group"),
         layout: &bind_group_layout,
         entries: &[],
      });

      let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
         label: Some("Immediate Geometry Shader"),
         source: wgpu::ShaderSource::Wgsl(include_str!("shader/solid.wgsl").into()),
      });

      let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
         label: Some("Immediate Geometry"),
         layout: Some(&pipeline_layout),
         vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "main_vs",
            buffers: &[wgpu::VertexBufferLayout {
               array_stride: (std::mem::size_of::<f32>() * 2) as wgpu::BufferAddress,
               step_mode: wgpu::VertexStepMode::Vertex,
               attributes: &[wgpu::VertexAttribute {
                  format: wgpu::VertexFormat::Float32x2,
                  offset: 0,
                  shader_location: 0,
               }],
            }],
         },
         primitive: wgpu::PrimitiveState::default(),
         fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "main_fs",
            targets: &[Some(swapchain_format.into())],
         }),
         depth_stencil: None,
         multisample: wgpu::MultisampleState::default(),
         multiview: None,
      });

      let mut renderer = Self {
         window,
         instance,
         surface,
         adapter,
         device,
         queue,

         vertex_buffer,
         bind_group,
         render_pipeline,
      };
      renderer.configure_surface();

      Ok(renderer)
   }

   fn configure_surface(&mut self) {
      let size = self.window.inner_size();
      let swapchain_capabilities = self.surface.get_capabilities(&self.adapter);
      let swapchain_format = swapchain_capabilities.formats[0];
      self.surface.configure(
         &self.device,
         &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: swapchain_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Mailbox,
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
         },
      );
   }

   pub fn window(&self) -> &Window {
      &self.window
   }
}

pub trait UiRenderFrame {
   fn render_frame(&mut self, f: impl FnOnce(&mut Self)) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<WgpuBackend> {
   fn render_frame(&mut self, f: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      let frame =
         self.surface.get_current_texture().context("Failed to acquire next swap chain texture")?;
      let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
      let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
         label: Some("Render Pass Encoder"),
      });
      {
         let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Hello Triangle"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
               view: &view,
               resolve_target: None,
               ops: wgpu::Operations {
                  load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                  store: true,
               },
            })],
            depth_stencil_attachment: None,
         });
         render_pass.set_pipeline(&self.render_pipeline);
         render_pass.set_bind_group(0, &self.bind_group, &[]);
         render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
         render_pass.draw(0..3, 0..1);
      }

      self.queue.submit([encoder.finish()]);
      frame.present();

      f(self);
      Ok(())
   }
}
