mod scene_uniforms;

use netcanv_renderer::BlendMode;
use tracing::{debug, info_span};
use winit::dpi::PhysicalSize;

pub use scene_uniforms::*;

/// Common GPU state.
pub struct Gpu {
   pub surface: wgpu::Surface,
   pub adapter: wgpu::Adapter,
   pub capabilities: wgpu::SurfaceCapabilities,
   pub device: wgpu::Device,
   pub queue: wgpu::Queue,

   pub linear_sampler: wgpu::Sampler,
   pub nearest_sampler: wgpu::Sampler,
   pub scene_uniform_bind_group_layout: wgpu::BindGroupLayout,

   pub screen_texture: wgpu::Texture,
   pub screen_texture_bind_group_layout: wgpu::BindGroupLayout,
   pub screen_texture_bind_group: wgpu::BindGroup,
   pub current_render_target: Option<wgpu::TextureView>,
   pub current_render_target_size: (u32, u32),
}

impl Gpu {
   pub fn create_screen_texture_view_and_bind_group(
      device: &wgpu::Device,
      bind_group_layout: &wgpu::BindGroupLayout,
      window_size: PhysicalSize<u32>,
   ) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
      let texture = device.create_texture(&wgpu::TextureDescriptor {
         label: Some("Screen Texture"),
         size: wgpu::Extent3d {
            width: window_size.width,
            height: window_size.height,
            depth_or_array_layers: 1,
         },
         mip_level_count: 1,
         sample_count: 1,
         dimension: wgpu::TextureDimension::D2,
         format: wgpu::TextureFormat::Rgba8Unorm,
         usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
         view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
      });
      let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
         label: Some("Screen Texture View"),
         ..Default::default()
      });
      let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some("Screen Bind Group"),
         layout: bind_group_layout,
         entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&texture_view),
         }],
      });
      (texture, texture_view, bind_group)
   }

   pub fn handle_resize(&mut self, window_size: PhysicalSize<u32>) {
      let _span = info_span!("handle_resize").entered();

      self.configure_surface(window_size);

      self.screen_texture.destroy();
      let (screen_texture, screen_texture_view, screen_texture_bind_group) =
         Self::create_screen_texture_view_and_bind_group(
            &self.device,
            &self.screen_texture_bind_group_layout,
            window_size,
         );
      self.screen_texture = screen_texture;
      self.screen_texture_bind_group = screen_texture_bind_group;
      self.current_render_target = Some(screen_texture_view);
      self.current_render_target_size = (window_size.width, window_size.height);
   }

   pub fn screen_format(&self) -> wgpu::TextureFormat {
      self.screen_texture.format()
   }

   pub fn surface_format(&self) -> wgpu::TextureFormat {
      self
         .capabilities
         .formats
         .iter()
         .find(|format| !format.is_srgb())
         .copied()
         .unwrap_or(self.capabilities.formats[0])
   }

   fn configure_surface(&self, size: PhysicalSize<u32>) {
      let surface_configuration = wgpu::SurfaceConfiguration {
         usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
         format: self.surface_format(),
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

   pub fn render_target(&self) -> &wgpu::TextureView {
      self.current_render_target.as_ref().expect("attempt to render outside of render_frame")
   }

   pub fn color_target_state(&self, blend_mode: BlendMode) -> wgpu::ColorTargetState {
      wgpu::ColorTargetState {
         format: self.screen_format(),
         blend: Some(match blend_mode {
            BlendMode::Replace => wgpu::BlendState::REPLACE,
            BlendMode::Alpha => wgpu::BlendState::ALPHA_BLENDING,
            BlendMode::Add => wgpu::BlendState {
               color: wgpu::BlendComponent {
                  src_factor: wgpu::BlendFactor::SrcAlpha,
                  operation: wgpu::BlendOperation::Add,
                  dst_factor: wgpu::BlendFactor::One,
               },
               alpha: wgpu::BlendComponent {
                  src_factor: wgpu::BlendFactor::SrcAlpha,
                  operation: wgpu::BlendOperation::Add,
                  dst_factor: wgpu::BlendFactor::One,
               },
            },
            BlendMode::Invert => wgpu::BlendState {
               color: wgpu::BlendComponent {
                  src_factor: wgpu::BlendFactor::OneMinusDst,
                  operation: wgpu::BlendOperation::Add,
                  dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
               },
               alpha: wgpu::BlendComponent {
                  src_factor: wgpu::BlendFactor::Zero,
                  operation: wgpu::BlendOperation::Add,
                  dst_factor: wgpu::BlendFactor::One,
               },
            },
         }),
         write_mask: wgpu::ColorWrites::ALL,
      }
   }
}
