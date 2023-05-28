use glam::vec2;
use gpu::{Gpu, Uniforms};
use immediate_geometry::{ImmediateGeometry, Vertex};
use netcanv_renderer::paws::Ui;

pub use winit;

mod error;
mod gpu;
mod immediate_geometry;
mod rendering;

use anyhow::Context;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use rendering::*;

pub struct WgpuBackend {
   window: Window,
   gpu: Gpu,

   // TODO: We should have this be event-driven instead of polling every frame.
   context_size: PhysicalSize<u32>,

   immediate_geometry: ImmediateGeometry,
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

      let (device, queue) = adapter.request_device(
         &wgpu::DeviceDescriptor {
            label: None,
            features: wgpu::Features::empty(),
            limits: wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits()),
         },
         None,
      ).await.context("Failed to acquire graphics device. Try updating your graphics drivers. If that doesn't work, your hardware may be too old to run NetCanv.")?;

      let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
         label: Some("Immediate Geometry Uniform Buffer"),
         size: std::mem::size_of::<Uniforms>() as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
         mapped_at_creation: false,
      });

      let gpu = Gpu {
         surface,
         adapter,
         device,
         queue,
         uniform_buffer,
      };
      gpu.handle_resize(window.inner_size());

      let context_size = window.inner_size();
      Ok(Self {
         immediate_geometry: ImmediateGeometry::new(&gpu),

         window,
         gpu,

         context_size,
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
         .context("Failed to acquire next swap chain texture")?;
      let frame_texture = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
      let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
         label: Some("Render Pass Encoder"),
      });
      {
         self.immediate_geometry.draw(
            "Hello Triangle",
            &self.gpu,
            &mut encoder,
            &frame_texture,
            &[
               Vertex {
                  position: vec2(0.0, 0.0),
               },
               Vertex {
                  position: vec2(32.0, 0.0),
               },
               Vertex {
                  position: vec2(0.0, 32.0),
               },
            ],
         );
      }
      self.gpu.queue.submit([encoder.finish()]);
      frame.present();

      f(self);
      Ok(())
   }
}
