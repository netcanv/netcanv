use std::collections::HashMap;
use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{uvec4, UVec4};
use swash::zeno::Placement;

use crate::gpu::Gpu;

pub struct GpuFont {
   atlas_texture: wgpu::Texture,
   glyph_data_buffer: wgpu::Buffer,
   pub bind_group: wgpu::BindGroup,

   glyph_allocator: guillotiere::SimpleAtlasAllocator,
   glyph_count: u32,

   glyph_cache: HashMap<(swash::GlyphId, u8), Option<(u32, Placement)>>,
}

impl GpuFont {
   const ATLAS_SIZE: u32 = 512;
   const MAX_GLYPHS: u32 = 16384 / size_of::<AtlasGlyph>() as u32;
   const DATA_BUFFER_SIZE: u32 = Self::MAX_GLYPHS * size_of::<AtlasGlyph>() as u32;

   pub fn new(gpu: &Gpu, bind_group_layout: &wgpu::BindGroupLayout, label: &str) -> Self {
      let atlas_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
         label: Some(&format!("{label}: Atlas Texture")),
         size: wgpu::Extent3d {
            width: Self::ATLAS_SIZE,
            height: Self::ATLAS_SIZE,
            depth_or_array_layers: 1,
         },
         mip_level_count: 1,
         sample_count: 1,
         dimension: wgpu::TextureDimension::D2,
         format: wgpu::TextureFormat::R8Unorm,
         usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
         view_formats: &[wgpu::TextureFormat::R8Unorm],
      });

      let glyph_data_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
         label: Some(&format!("{label}: Glyph Atlas Data Buffer")),
         size: Self::DATA_BUFFER_SIZE as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
         mapped_at_creation: false,
      });

      let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some(&format!("{label}: Bind Group")),
         layout: bind_group_layout,
         entries: &[
            wgpu::BindGroupEntry {
               binding: 0,
               resource: wgpu::BindingResource::TextureView(&atlas_texture.create_view(
                  &wgpu::TextureViewDescriptor {
                     label: Some(&format!("{label}: Texture View")),
                     ..Default::default()
                  },
               )),
            },
            wgpu::BindGroupEntry {
               binding: 1,
               resource: glyph_data_buffer.as_entire_binding(),
            },
         ],
      });

      Self {
         atlas_texture,
         glyph_data_buffer,
         bind_group,
         glyph_allocator: guillotiere::SimpleAtlasAllocator::new(guillotiere::size2(
            Self::ATLAS_SIZE as i32,
            Self::ATLAS_SIZE as i32,
         )),
         glyph_count: 0,
         glyph_cache: HashMap::new(),
      }
   }

   pub fn get_or_upload_glyph<'a>(
      &mut self,
      gpu: &Gpu,
      glyph_id: swash::GlyphId,
      subposition: u8,
      glyph: impl FnOnce() -> Option<(Placement, Vec<u8>)>,
   ) -> Option<(u32, Placement)> {
      if let Some(cached) = self.glyph_cache.get(&(glyph_id, subposition)) {
         *cached
      } else {
         let id = glyph().and_then(|(placement, data)| {
            self
               .upload_glyph(gpu, placement.width, placement.height, &data)
               .map(|glyph_index| (glyph_index, placement))
         });
         self.glyph_cache.insert((glyph_id, subposition), id);
         id
      }
   }

   fn upload_glyph(&mut self, gpu: &Gpu, width: u32, height: u32, data: &[u8]) -> Option<u32> {
      if let Some(rect) =
         self.glyph_allocator.allocate(guillotiere::size2(width as i32, height as i32))
      {
         if self.glyph_count < Self::MAX_GLYPHS {
            let index = self.glyph_count;
            self.glyph_count += 1;
            let atlas_glyph = AtlasGlyph {
               rect: uvec4(
                  rect.min.x as u32,
                  rect.min.y as u32,
                  rect.width() as u32,
                  rect.height() as u32,
               ),
            };
            gpu.queue.write_buffer(
               &self.glyph_data_buffer,
               (index as usize * size_of::<AtlasGlyph>()) as wgpu::BufferAddress,
               bytemuck::bytes_of(&atlas_glyph),
            );
            gpu.queue.write_texture(
               wgpu::ImageCopyTexture {
                  texture: &self.atlas_texture,
                  mip_level: 0,
                  origin: wgpu::Origin3d {
                     x: rect.min.x as u32,
                     y: rect.min.y as u32,
                     z: 0,
                  },
                  aspect: wgpu::TextureAspect::All,
               },
               data,
               wgpu::ImageDataLayout {
                  offset: 0,
                  bytes_per_row: Some(rect.width() as u32),
                  rows_per_image: None,
               },
               wgpu::Extent3d {
                  width: rect.width() as u32,
                  height: rect.height() as u32,
                  depth_or_array_layers: 1,
               },
            );
            Some(index)
         } else {
            None
         }
      } else {
         None
      }
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
pub struct AtlasGlyph {
   pub rect: UVec4,
}

unsafe impl Zeroable for AtlasGlyph {}
unsafe impl Pod for AtlasGlyph {}
