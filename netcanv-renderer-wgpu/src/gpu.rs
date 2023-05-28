use bytemuck::{Pod, Zeroable};
use glam::{vec3a, Mat3A};
use winit::dpi::PhysicalSize;

/// Common GPU state.
pub struct Gpu {
   pub surface: wgpu::Surface,
   pub adapter: wgpu::Adapter,
   pub device: wgpu::Device,
   pub queue: wgpu::Queue,

   pub uniform_buffer: wgpu::Buffer,
}

impl Gpu {
   pub fn handle_resize(&self, window_size: PhysicalSize<u32>) {
      self.configure_surface(window_size);
      self.upload_uniforms(window_size);
   }

   fn configure_surface(&self, size: PhysicalSize<u32>) {
      let capabilities = self.surface.get_capabilities(&self.adapter);
      let format = capabilities.formats[0];
      self.surface.configure(
         &self.device,
         &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            // Choose the mode that has the lowest latency, because noone likes it when their
            // brush acts all floaty.
            present_mode: if capabilities.present_modes.contains(&wgpu::PresentMode::Mailbox) {
               wgpu::PresentMode::Mailbox
            } else {
               wgpu::PresentMode::AutoVsync
            },
            alpha_mode: wgpu::CompositeAlphaMode::Opaque,
            view_formats: vec![],
         },
      );
   }

   fn upload_uniforms(&self, window_size: PhysicalSize<u32>) {
      let width = window_size.width as f32;
      let height = window_size.height as f32;

      self.queue.write_buffer(
         &self.uniform_buffer,
         0,
         bytemuck::bytes_of(&Uniforms {
            transform: Mat3A::from_cols(
               vec3a(2.0 / width, 0.0, 0.0),
               vec3a(0.0, -2.0 / height, 0.0),
               vec3a(-1.0, 1.0, 0.0),
            ),
         }),
      )
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct Uniforms {
   pub transform: Mat3A,
}

unsafe impl Zeroable for Uniforms {}
unsafe impl Pod for Uniforms {}
