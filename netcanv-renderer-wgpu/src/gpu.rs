use std::cell::Cell;

use bytemuck::{Pod, Zeroable};
use glam::{vec3a, Mat3A};
use log::debug;
use winit::dpi::PhysicalSize;

/// Common GPU state.
pub struct Gpu {
   pub surface: wgpu::Surface,
   pub adapter: wgpu::Adapter,
   pub capabilities: wgpu::SurfaceCapabilities,
   pub device: wgpu::Device,
   pub queue: wgpu::Queue,

   pub scene_uniform_buffer: wgpu::Buffer,
   pub scene_uniform_bind_group_layout: wgpu::BindGroupLayout,
   pub scene_uniform_bind_group: wgpu::BindGroup,

   pub current_render_target: Option<wgpu::TextureView>,

   pub depth_index_counter: Cell<u32>,
}

impl Gpu {
   pub fn handle_resize(&mut self, window_size: PhysicalSize<u32>) {
      self.configure_surface(window_size);
      self.update_scene_uniforms(window_size);
   }

   pub fn surface_format(&self) -> wgpu::TextureFormat {
      self
         .capabilities
         .formats
         .iter()
         // NOTE: Since our render results are in sRGB already, we don't want to perform any
         // automatic conversion work on that after the fact.
         .find(|format| !format.is_srgb())
         .copied()
         .unwrap_or(self.capabilities.formats[0])
   }

   fn configure_surface(&self, size: PhysicalSize<u32>) {
      let format = self.surface_format();
      let surface_configuration = wgpu::SurfaceConfiguration {
         usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
         format,
         width: size.width,
         height: size.height,
         // Choose the mode that has the lowest latency, because noone likes it when their
         // brush acts all floaty.
         present_mode: if self.capabilities.present_modes.contains(&wgpu::PresentMode::Mailbox) {
            wgpu::PresentMode::Mailbox
         } else {
            wgpu::PresentMode::AutoVsync
         },
         alpha_mode: wgpu::CompositeAlphaMode::Opaque,
         view_formats: vec![],
      };
      debug!("using surface format: {surface_configuration:?}");
      self.surface.configure(&self.device, &surface_configuration);
   }

   fn update_scene_uniforms(&self, window_size: PhysicalSize<u32>) {
      let width = window_size.width as f32;
      let height = window_size.height as f32;

      self.queue.write_buffer(
         &self.scene_uniform_buffer,
         0,
         bytemuck::bytes_of(&SceneUniforms {
            transform: Mat3A::from_cols(
               vec3a(2.0 / width, 0.0, 0.0),
               vec3a(0.0, -2.0 / height, 0.0),
               vec3a(-1.0, 1.0, 0.0),
            ),
         }),
      )
   }

   pub fn next_depth_index(&self) -> u32 {
      let index = self.depth_index_counter.get();
      self.depth_index_counter.set(index.saturating_add(1));
      index
   }

   pub fn render_target(&self) -> &wgpu::TextureView {
      self.current_render_target.as_ref().expect("attempt to render outside of render_frame")
   }

   pub fn color_target_state(&self) -> wgpu::ColorTargetState {
      wgpu::ColorTargetState {
         format: self.surface_format(),
         blend: Some(wgpu::BlendState {
            color: wgpu::BlendComponent::OVER,
            alpha: wgpu::BlendComponent::OVER,
         }),
         write_mask: wgpu::ColorWrites::ALL,
      }
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SceneUniforms {
   pub transform: Mat3A,
}

unsafe impl Zeroable for SceneUniforms {}
unsafe impl Pod for SceneUniforms {}
