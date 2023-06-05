use std::cell::Cell;

use anyhow::Context;
use cli::RendererCli;
use gpu::{Gpu, SceneUniforms};
use image::ImageStorage;
use netcanv_renderer::paws::{Color, Ui};
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use winit;

mod batch_storage;
pub mod cli;
mod common;
mod gpu;
mod image;
mod pass;
mod rendering;

pub use image::*;
pub use rendering::*;

pub struct WgpuBackend {
   window: Window,
   gpu: Gpu,

   // TODO: We should have this be event-driven instead of polling every frame.
   context_size: PhysicalSize<u32>,

   image_storage: ImageStorage,

   clear: Option<Color>,
   rounded_rects: pass::RoundedRects,
   lines: pass::Lines,
   images: pass::Images,

   command_buffers: Vec<wgpu::CommandBuffer>,
}

impl WgpuBackend {
   pub async fn new(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
      cli: &RendererCli,
   ) -> anyhow::Result<Self> {
      let window = window_builder.build(event_loop).context("Failed to create window")?;
      let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
         backends: if let Some(backend) = cli.wgpu_backend {
            wgpu::Backends::from(backend)
         } else {
            wgpu::Backends::all()
         },
         ..Default::default()
      });

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

      let capabilities = surface.get_capabilities(&adapter);
      log::info!("adapter capabilities: {capabilities:#?}");
      log::info!("adapter limits: {:#?}", adapter.limits());

      let (device, queue) = adapter.request_device(
         &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
         },
         None,
      ).await.context("Failed to acquire graphics device. Try updating your graphics drivers. If that doesn't work, your hardware may be too old to run NetCanv.")?;

      let scene_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
         label: Some("Scene Uniform Buffer"),
         size: std::mem::size_of::<SceneUniforms>() as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
         mapped_at_creation: false,
      });
      let image_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
         label: Some("Image Sampler"),
         address_mode_u: wgpu::AddressMode::ClampToEdge,
         address_mode_v: wgpu::AddressMode::ClampToEdge,
         address_mode_w: wgpu::AddressMode::ClampToEdge,
         mag_filter: wgpu::FilterMode::Linear,
         min_filter: wgpu::FilterMode::Linear,
         ..Default::default()
      });

      let scene_uniform_bind_group_layout =
         device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Scene Uniform Bind Group Layout"),
            entries: &[
               wgpu::BindGroupLayoutEntry {
                  binding: 0,
                  visibility: wgpu::ShaderStages::VERTEX,
                  ty: wgpu::BindingType::Buffer {
                     ty: wgpu::BufferBindingType::Uniform,
                     has_dynamic_offset: false,
                     min_binding_size: None,
                  },
                  count: None,
               },
               wgpu::BindGroupLayoutEntry {
                  binding: 1,
                  visibility: wgpu::ShaderStages::FRAGMENT,
                  ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                  count: None,
               },
            ],
         });
      let scene_uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some("Scene Uniform Bind Group"),
         layout: &scene_uniform_bind_group_layout,
         entries: &[
            wgpu::BindGroupEntry {
               binding: 0,
               resource: scene_uniform_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
               binding: 1,
               resource: wgpu::BindingResource::Sampler(&image_sampler),
            },
         ],
      });

      let depth_buffer = Gpu::create_depth_buffer(&device, window.inner_size());
      let depth_buffer_view = depth_buffer.create_view(&wgpu::TextureViewDescriptor::default());

      let mut gpu = Gpu {
         surface,
         adapter,
         capabilities,
         device,
         queue,
         scene_uniform_buffer,
         scene_uniform_bind_group_layout,
         scene_uniform_bind_group,
         depth_buffer,
         depth_buffer_view,
         current_render_target: None,
         depth_index_counter: Cell::new(0),
      };
      gpu.handle_resize(window.inner_size());

      let image_storage = ImageStorage::new(&gpu);

      let context_size = window.inner_size();
      Ok(Self {
         rounded_rects: pass::RoundedRects::new(&gpu),
         lines: pass::Lines::new(&gpu),
         images: pass::Images::new(&gpu, &image_storage),

         image_storage,

         window,
         gpu,

         clear: None,
         context_size,
         command_buffers: vec![],
      })
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
      let window_size = self.window.inner_size();
      if self.context_size != window_size {
         self.gpu.handle_resize(window_size);
         self.context_size = window_size;
      }

      let frame = self
         .gpu
         .surface
         .get_current_texture()
         .context("Failed to acquire next swapchain texture")?;
      let frame_texture = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
      self.gpu.current_render_target = Some(frame_texture);
      self.gpu.depth_index_counter.set(0);

      self.rewind();
      f(self);
      self.flush();

      {
         // Slight borrow checker hack here because borrowing out individual fields doesn't work
         // through Deref.
         let backend = self.render();
         backend.gpu.queue.submit(backend.command_buffers.drain(..));
      }

      frame.present();

      Ok(())
   }
}
