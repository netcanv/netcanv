use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{vec4, Vec4};
use netcanv_renderer::paws::{Color, Rect};
use wgpu::util::DeviceExt;

use crate::batch_storage::{BatchStorage, BatchStorageConfig};
use crate::image::ImageStorage;
use crate::{ClearOps, FlushContext, Image};

use super::vertex::{vertex, Vertex};
use super::PassCreationContext;

pub(crate) struct Images {
   vertex_buffer: wgpu::Buffer,
   batch_storage: BatchStorage,
   render_pipeline: wgpu::RenderPipeline,

   image_rect_data: Vec<ImageRectData>,
   image_bindings: Vec<u32>,
}

impl Images {
   const RESERVED_RECT_COUNT: usize = 512;

   pub fn new(context: &PassCreationContext<'_>, image_storage: &ImageStorage) -> Self {
      let shader =
         context.gpu.device.create_shader_module(wgpu::include_wgsl!("shader/images.wgsl"));

      let vertex_buffer =
         context.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Images: Vertex Buffer"),
            contents: bytemuck::cast_slice(&[
               vertex(1.0, 1.0),
               vertex(0.0, 1.0),
               vertex(0.0, 0.0),
               vertex(1.0, 1.0),
               vertex(1.0, 0.0),
               vertex(0.0, 0.0),
            ]),
            usage: wgpu::BufferUsages::VERTEX,
         });

      let image_rect_data_bind_group_layout =
         context.gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Images: Data Buffer Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
               binding: 0,
               visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
               ty: wgpu::BindingType::Buffer {
                  ty: wgpu::BufferBindingType::Uniform,
                  has_dynamic_offset: false,
                  min_binding_size: None,
               },
               count: None,
            }],
         });

      let pipeline_layout =
         context.gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Images: Render Pipeline Layout"),
            bind_group_layouts: &[
               &image_storage.bind_group_layout,
               &image_rect_data_bind_group_layout,
               context.model_transform_bind_group_layout,
               &context.gpu.scene_uniform_bind_group_layout,
            ],
            push_constant_ranges: &[],
         });
      let render_pipeline =
         context.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Images: Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
               module: &shader,
               entry_point: "main_vs",
               buffers: &[Vertex::LAYOUT],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(context.gpu.depth_stencil_state()),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
               module: &shader,
               entry_point: "main_fs",
               targets: &[Some(context.gpu.color_target_state())],
            }),
            multiview: None,
         });

      Self {
         vertex_buffer,
         batch_storage: BatchStorage::new(BatchStorageConfig {
            name: "Images",
            buffer_size: (size_of::<ImageRectData>() * Self::RESERVED_RECT_COUNT)
               as wgpu::BufferAddress,
            bind_group_layout: image_rect_data_bind_group_layout,
         }),
         render_pipeline,
         image_rect_data: Vec::with_capacity(Self::RESERVED_RECT_COUNT),
         image_bindings: Vec::with_capacity(Self::RESERVED_RECT_COUNT),
      }
   }

   pub fn add(&mut self, depth_index: u32, rect: Rect, image: &Image) {
      assert!(
         self.image_rect_data.len() < self.image_rect_data.capacity(),
         "too many images without flushing"
      );

      self.image_rect_data.push(ImageRectData {
         rect: vec4(rect.x(), rect.y(), rect.width(), rect.height()),
         depth_index,
         color: image.color.unwrap_or(Color::TRANSPARENT),
         colorize: image.color.is_some() as u32,
      });
      self.image_bindings.push(image.index);
   }

   pub fn flush(&mut self, context: &mut FlushContext<'_>, image_storage: &ImageStorage) {
      // TODO: This should interact with clearing, probably.
      if self.image_rect_data.is_empty() {
         return;
      }

      let (image_rect_data_buffer, bind_group) = self.batch_storage.next_batch(context.gpu);

      let image_rect_data_bytes = bytemuck::cast_slice(&self.image_rect_data);
      context.gpu.queue.write_buffer(image_rect_data_buffer, 0, image_rect_data_bytes);

      let ClearOps { color, depth } = context.clear_ops.take();
      let mut render_pass = context.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
         label: Some("Images"),
         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: context.gpu.render_target(),
            resolve_target: None,
            ops: color,
         })],
         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &context.gpu.depth_buffer_view,
            depth_ops: Some(depth),
            stencil_ops: None,
         }),
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.set_bind_group(1, bind_group, &[]);
      render_pass.set_bind_group(2, context.model_transform_bind_group, &[]);
      render_pass.set_bind_group(3, &context.gpu.scene_uniform_bind_group, &[]);
      for (i, &image_index) in self.image_bindings.iter().enumerate() {
         let i = i as u32;
         render_pass.set_bind_group(
            0,
            &image_storage.images[image_index as usize].bind_group,
            &[],
         );
         render_pass.draw(0..6, i..i + 1);
      }

      self.image_rect_data.clear();
      self.image_bindings.clear();
   }

   pub fn needs_flush(&self) -> bool {
      self.image_rect_data.len() >= self.image_rect_data.capacity()
   }

   pub fn rewind(&mut self) {
      self.batch_storage.rewind();
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct ImageRectData {
   rect: Vec4,
   depth_index: u32,
   color: Color,
   colorize: u32,
}

unsafe impl Pod for ImageRectData {}
unsafe impl Zeroable for ImageRectData {}
