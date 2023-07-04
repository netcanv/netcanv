use std::cell::Cell;
use std::sync::{Arc, OnceLock};

use log::warn;
use netcanv_renderer::ScalingFilter;

use crate::gpu::Gpu;
use crate::image::ImageStorage;

pub struct Framebuffer {
   width: u32,
   height: u32,
   texture: wgpu::Texture,
   pub(crate) texture_view: Cell<Option<wgpu::TextureView>>,
   pub(crate) image_storage_index: u32,
   pub(crate) scaling_filter: ScalingFilter,
}

impl Framebuffer {
   pub(crate) fn new(gpu: &Gpu, image_storage: &mut ImageStorage, width: u32, height: u32) -> Self {
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
         dimension: wgpu::TextureDimension::D2,
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

      let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some(&format!("{label}: Bind Group")),
         layout: &image_storage.bind_group_layout,
         entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::TextureView(&texture_view),
         }],
      });
      let image_storage_index = image_storage.add_external(bind_group);

      Self {
         width,
         height,
         texture,
         texture_view: Cell::new(Some(texture_view)),
         image_storage_index,
         scaling_filter: ScalingFilter::default(),
      }
   }

   pub(crate) fn upload(&self, gpu: &Gpu, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {
      let (x, y) = position;
      let (width, height) = size;
      gpu.queue.write_texture(
         wgpu::ImageCopyTextureBase {
            texture: &self.texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
         },
         pixels,
         wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: None,
         },
         wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
         },
      )
   }

   pub(crate) fn sync_download(
      &self,
      gpu: &Gpu,
      position: (u32, u32),
      size: (u32, u32),
      out_pixels: &mut [u8],
   ) {
      // This is pretty disgusting, but I didn't wanna rewrite the entire rendering API for
      // asynchronous download support. Someday this will become a truly asynchronous process,
      // but today is not that day.
      let (x, y) = position;
      let (width, height) = size;

      let packed_bytes_per_row = width * 4;
      const ALIGNMENT: u32 = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
      // This gets optimized down to bit twiddling, so don't worry about the division.
      let aligned_bytes_per_row = (packed_bytes_per_row + ALIGNMENT - 1) / ALIGNMENT * ALIGNMENT;

      let download_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
         label: Some("(temporary) Framebuffer Pixel Download Buffer"),
         size: (aligned_bytes_per_row * height) as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
         mapped_at_creation: false,
      });
      let mut command_encoder =
         gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("(temporary) Framebuffer Pixel Download Command Encoder"),
         });
      command_encoder.copy_texture_to_buffer(
         wgpu::ImageCopyTexture {
            texture: &self.texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
         },
         wgpu::ImageCopyBuffer {
            buffer: &download_buffer,
            layout: wgpu::ImageDataLayout {
               offset: 0,
               bytes_per_row: Some(aligned_bytes_per_row),
               rows_per_image: None,
            },
         },
         wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
         },
      );
      let index = gpu.queue.submit([command_encoder.finish()]);

      // And this is the terrible part.
      let cell = Arc::new(OnceLock::new());
      let slice = download_buffer.slice(..);
      slice.map_async(wgpu::MapMode::Read, {
         let cell = Arc::clone(&cell);
         move |result| cell.set(result).expect("cell must only be set once")
      });
      // Blocking here won't work on web, which is why this process has to become async.
      gpu.device.poll(wgpu::Maintain::WaitForSubmissionIndex(index));
      match cell.get() {
         Some(Ok(_)) => {
            let pixels = &slice.get_mapped_range()[..];
            if packed_bytes_per_row == aligned_bytes_per_row {
               // Fast path: we don't need to account for spacing between rows, we can just copy
               // all the pixels in one go.
               out_pixels.copy_from_slice(pixels);
            } else {
               // Slow path: the downloaded buffer has padding between rows and we have to account
               // for that by copying the result row by row.
               for y in 0..height {
                  let src_start = (y * aligned_bytes_per_row) as usize;
                  let src_end = src_start + packed_bytes_per_row as usize;
                  let dst_start = (y  * packed_bytes_per_row) as usize;
                  let dst_end = dst_start + packed_bytes_per_row as usize;
                  out_pixels[dst_start..dst_end].copy_from_slice(&pixels[src_start..src_end]);
               }
            }
         }
         Some(Err(error)) => warn!("could not copy pixels from the CPU to the GPU: {error}"),
         None => warn!("could not copy pixels from the CPU to the GPU: did not receive a signal that the pixels are ready"),
      }
      download_buffer.destroy();
   }
}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }

   fn set_scaling_filter(&mut self, filter: ScalingFilter) {
      self.scaling_filter = filter;
   }
}
