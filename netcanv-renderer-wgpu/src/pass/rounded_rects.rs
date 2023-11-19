use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{vec4, Vec4};
use netcanv_renderer::paws::{Color, Rect};
use wgpu::include_wgsl;
use wgpu::util::DeviceExt;

use crate::batch_storage::{BatchStorage, BatchStorageConfig};
use crate::rendering::{BlendFlags, FlushContext};

use super::vertex::{vertex, Vertex};
use super::{PassCreationContext, RenderPipelinePermutations};

/// Pipeline for drawing rounded rectangles.
pub(crate) struct RoundedRects {
   vertex_buffer: wgpu::Buffer,
   batch_storage: BatchStorage,
   render_pipelines: RenderPipelinePermutations,

   rect_data: Vec<RectData>,
}

impl RoundedRects {
   const RESERVED_RECT_COUNT: usize = 512;

   pub fn new(context: &PassCreationContext<'_>) -> Self {
      let shader =
         context.gpu.device.create_shader_module(include_wgsl!("shader/rounded_rects.wgsl"));

      let vertex_buffer =
         context.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("RoundedRects: Vertex Buffer"),
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

      let bind_group_layout =
         context.gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RoundedRects: Bind Group Layout"),
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
            label: Some("RoundedRects: Pipeline Layout"),
            bind_group_layouts: &[
               &bind_group_layout,
               context.model_transform_bind_group_layout,
               &context.gpu.scene_uniform_bind_group_layout,
            ],
            push_constant_ranges: &[],
         });
      let render_pipelines = RenderPipelinePermutations::new(|label, blend_mode| {
         context.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("RoundedRects: Render Pipeline {label}")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
               module: &shader,
               entry_point: "main_vs",
               buffers: &[Vertex::LAYOUT],
            },
            primitive: wgpu::PrimitiveState::default(),
            fragment: Some(wgpu::FragmentState {
               module: &shader,
               entry_point: "main_fs",
               targets: &[Some(context.gpu.color_target_state(blend_mode))],
            }),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
         })
      });

      Self {
         vertex_buffer,
         batch_storage: BatchStorage::new(BatchStorageConfig {
            name: "RoundedRects",
            buffer_size: (size_of::<RectData>() * Self::RESERVED_RECT_COUNT) as wgpu::BufferAddress,
            bind_group_layout,
         }),
         render_pipelines,
         rect_data: Vec::with_capacity(Self::RESERVED_RECT_COUNT),
      }
   }

   pub fn add(
      &mut self,
      rect: Rect,
      color: Color,
      corner_radius: f32,
      outline: f32,
      blend_flags: BlendFlags,
   ) {
      assert!(
         self.rect_data.len() < self.rect_data.capacity(),
         "too many rectangles without flushing"
      );

      // A slightly janky way of ensuring that UI rects look laser-sharp while single point
      // brush strokes on the paint canvas are placed as-is. Not the most ideal way of accomplishing
      // this, but it works and is pretty fast.
      let mut rect = vec4(rect.left(), rect.top(), rect.width(), rect.height());
      if blend_flags.contains(BlendFlags::ANTIALIAS) {
         rect = rect.floor();
      }

      self.rect_data.push(RectData {
         color,
         rect,
         corner_radius,
         outline,
         rendition: blend_flags.bits(),
      });
   }

   pub fn flush<'a>(
      &'a mut self,
      context: &mut FlushContext<'a>,
      render_pass: &mut wgpu::RenderPass<'a>,
   ) {
      profiling::scope!("RoundedRects::flush");

      // TODO: This should interact with clearing, probably.
      if self.rect_data.is_empty() {
         return;
      }

      let (rect_data_buffer, bind_group) = self.batch_storage.next_batch(context.gpu);

      let rect_data_bytes = bytemuck::cast_slice(&self.rect_data);
      context.gpu.queue.write_buffer(rect_data_buffer, 0, rect_data_bytes);

      render_pass.push_debug_group("RoundedRects");
      render_pass.set_pipeline(self.render_pipelines.get(context.blend_mode));
      render_pass.set_bind_group(0, bind_group, &[]);
      render_pass.set_bind_group(1, context.model_transform_bind_group, &[]);
      render_pass.set_bind_group(2, context.scene_uniform_bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..6, 0..self.rect_data.len() as u32);
      render_pass.pop_debug_group();

      self.rect_data.clear();
   }

   pub fn needs_flush(&self) -> bool {
      self.rect_data.len() == self.rect_data.capacity()
   }

   pub fn rewind(&mut self) {
      self.batch_storage.rewind();
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct RectData {
   rect: Vec4,
   corner_radius: f32,
   color: Color,
   /// This being <= 0 means we should fill in the rectangle.
   outline: f32,
   rendition: u32,
}

impl Default for RectData {
   fn default() -> Self {
      Self {
         rect: Default::default(),
         corner_radius: Default::default(),
         color: Color::TRANSPARENT,
         outline: 0.0,
         rendition: 0,
      }
   }
}

unsafe impl Zeroable for RectData {}
unsafe impl Pod for RectData {}
