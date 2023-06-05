use netcanv_renderer::paws::Color;
use wgpu::util::DeviceExt;

use crate::gpu::Gpu;
use crate::WgpuBackend;

#[derive(Debug, Clone, Copy)]
pub struct Image {
   width: u32,
   height: u32,
   pub(crate) index: u32,
   pub(crate) color: Option<Color>,
}

impl netcanv_renderer::Image for Image {
   fn colorized(&self, color: Color) -> Self {
      Self {
         color: Some(color),
         ..*self
      }
   }

   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }
}

// NOTE(liquidev): Right now the implementation of images is quite rudimentary, having one texture
// per image; this could be improved by storing the images in an atlas, but I didn't really wanna
// get into the whole can of worms that is texture packing.

pub(crate) struct GpuImage {
   pub bind_group: wgpu::BindGroup,
}

pub(crate) struct ImageStorage {
   pub images: Vec<GpuImage>,
   pub bind_group_layout: wgpu::BindGroupLayout,
}

impl ImageStorage {
   pub fn new(gpu: &Gpu) -> Self {
      let bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Image Texture Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
               binding: 0,
               visibility: wgpu::ShaderStages::FRAGMENT,
               ty: wgpu::BindingType::Texture {
                  sample_type: wgpu::TextureSampleType::Float { filterable: true },
                  view_dimension: wgpu::TextureViewDimension::D2,
                  multisampled: false,
               },
               count: None,
            }],
         });

      Self {
         images: vec![],
         bind_group_layout,
      }
   }

   fn upload(&mut self, gpu: &Gpu, width: u32, height: u32, pixel_data: &[u8]) -> u32 {
      let index = self.images.len() as u32;

      let texture = gpu.device.create_texture_with_data(
         &gpu.queue,
         &wgpu::TextureDescriptor {
            label: Some(&format!("Image #{index} {width}x{height} Texture")),
            size: wgpu::Extent3d {
               width,
               height,
               depth_or_array_layers: 1,
            },
            // TODO: Mipmaps?
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[wgpu::TextureFormat::Rgba8Unorm],
         },
         pixel_data,
      );
      let texture_view = texture.create_view(&wgpu::TextureViewDescriptor {
         label: Some(&format!("Image #{index} {width}x{height} Texture View")),
         ..Default::default()
      });
      let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some(&format!("Image #{index} {width}x{height} Bind Group")),
         layout: &self.bind_group_layout,
         entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&texture_view),
         }],
      });

      self.images.push(GpuImage { bind_group });

      index
   }
}

impl WgpuBackend {
   pub(crate) fn create_image_from_rgba_impl(
      &mut self,
      width: u32,
      height: u32,
      pixel_data: &[u8],
   ) -> Image {
      assert!(
         pixel_data.len() & 3 == 0,
         "length pixel data must be a multiple of 4"
      );

      let index = self.image_storage.upload(&self.gpu, width, height, pixel_data);
      Image {
         width,
         height,
         index,
         color: None,
      }
   }
}
