use std::collections::HashMap;

use bytemuck::{Pod, Zeroable};
use glam::{vec3a, Mat3A};
use wgpu::util::DeviceExt;

use super::Gpu;

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SceneUniformData {
   pub transform: Mat3A,
}

unsafe impl Zeroable for SceneUniformData {}
unsafe impl Pod for SceneUniformData {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CacheKey {
   viewport_width: u32,
   viewport_height: u32,
}

struct CacheEntry {
   buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   eviction_timer: usize,
}

pub struct SceneUniformCache {
   cache: HashMap<CacheKey, CacheEntry>,
   eviction_time: usize, // in frames
}

impl SceneUniformCache {
   pub fn new(eviction_time: usize) -> Self {
      Self {
         cache: HashMap::new(),
         eviction_time,
      }
   }

   pub fn bind_group(&mut self, gpu: &Gpu, viewport_size: (u32, u32)) -> &wgpu::BindGroup {
      let (viewport_width, viewport_height) = viewport_size;
      &self
         .cache
         .entry(CacheKey {
            viewport_width,
            viewport_height,
         })
         .or_insert_with(|| {
            let buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
               label: Some(&format!(
                  "Scene Uniform Buffer ({viewport_width}x{viewport_height} viewport)"
               )),
               usage: wgpu::BufferUsages::UNIFORM,
               contents: bytemuck::bytes_of(&SceneUniformData {
                  transform: Mat3A::from_cols(
                     vec3a(2.0 / viewport_width as f32, 0.0, 0.0),
                     vec3a(0.0, -2.0 / viewport_height as f32, 0.0),
                     vec3a(-1.0, 1.0, 0.0),
                  ),
               }),
            });
            let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
               label: Some(&format!(
                  "Scene Uniform Bind Group ({viewport_width}x{viewport_height} viewport)"
               )),
               layout: &gpu.scene_uniform_bind_group_layout,
               entries: &[
                  wgpu::BindGroupEntry {
                     binding: 0,
                     resource: buffer.as_entire_binding(),
                  },
                  wgpu::BindGroupEntry {
                     binding: 1,
                     resource: wgpu::BindingResource::Sampler(&gpu.linear_sampler),
                  },
                  wgpu::BindGroupEntry {
                     binding: 2,
                     resource: wgpu::BindingResource::Sampler(&gpu.nearest_sampler),
                  },
               ],
            });
            CacheEntry {
               buffer,
               bind_group,
               eviction_timer: self.eviction_time,
            }
         })
         .bind_group
   }

   pub fn tick_and_evict(&mut self) {
      self.cache.retain(|_, entry| {
         entry.eviction_timer -= 0;
         if entry.eviction_timer == 0 {
            entry.buffer.destroy();
            false
         } else {
            true
         }
      })
   }
}
