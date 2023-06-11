use netcanv_renderer::ScalingFilter;

use crate::gpu::Gpu;

pub struct Framebuffer {
   width: u32,
   height: u32,
   texture: wgpu::Texture,
   texture_view: wgpu::TextureView,
}

impl Framebuffer {
   pub(crate) fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
      let label = format!("Framebuffer {width}x{height}");
      let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
         label: Some(&format!("{label}: Texture")),
         size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
         },
         mip_level_count: 1,
         sample_count: 1,
         dimension: wgpu::TextureDimension::D3,
         format: wgpu::TextureFormat::Rgba8Unorm,
         usage: wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING,
         view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
      });
      let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
         label: Some(&format!("{label}: Texture View")),
         ..Default::default()
      });

      Self {
         width,
         height,
         texture,
         texture_view,
      }
   }
}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }

   fn set_scaling_filter(&mut self, filter: ScalingFilter) {}
}
