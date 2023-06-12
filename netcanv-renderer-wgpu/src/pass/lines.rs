use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{vec4, Vec4};
use netcanv_renderer::paws::{Color, LineCap, Point};
use wgpu::util::DeviceExt;

use crate::batch_storage::{BatchStorage, BatchStorageConfig};
use crate::rendering::FlushContext;

use super::vertex::{vertex, Vertex};
use super::{PassCreationContext, RenderPipelinePermutations};

pub(crate) struct Lines {
   vertex_buffer: wgpu::Buffer,
   batch_storage: BatchStorage,
   render_pipelines: RenderPipelinePermutations,

   line_data: Vec<LineData>,
}

impl Lines {
   pub const RESERVED_LINE_COUNT: usize = 512;

   pub fn new(context: &PassCreationContext<'_>) -> Self {
      let shader =
         context.gpu.device.create_shader_module(wgpu::include_wgsl!("shader/lines.wgsl"));

      let vertex_buffer =
         context.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Lines: Vertex Buffer"),
            contents: bytemuck::cast_slice(&[
               vertex(0.0, 0.5),
               vertex(1.0, -0.5),
               vertex(1.0, 0.5),
               vertex(0.0, 0.5),
               vertex(0.0, -0.5),
               vertex(1.0, -0.5),
            ]),
            usage: wgpu::BufferUsages::VERTEX,
         });

      let bind_group_layout =
         context.gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lines: Bind Group Layout"),
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
            label: Some("Lines: Render Pipeline Layout"),
            bind_group_layouts: &[
               &bind_group_layout,
               context.model_transform_bind_group_layout,
               &context.gpu.scene_uniform_bind_group_layout,
            ],
            push_constant_ranges: &[],
         });
      let render_pipelines = RenderPipelinePermutations::new(|label, blend_mode| {
         context.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Lines: Render Pipeline {label}")),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
               module: &shader,
               entry_point: "main_vs",
               buffers: &[Vertex::LAYOUT],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
               module: &shader,
               entry_point: "main_fs",
               targets: &[Some(context.gpu.color_target_state(blend_mode))],
            }),
            multiview: None,
         })
      });

      Self {
         vertex_buffer,
         batch_storage: BatchStorage::new(BatchStorageConfig {
            name: "Lines",
            buffer_size: (size_of::<LineData>() * Self::RESERVED_LINE_COUNT) as wgpu::BufferAddress,
            bind_group_layout,
         }),
         render_pipelines,
         line_data: Vec::with_capacity(Self::RESERVED_LINE_COUNT),
      }
   }

   pub fn add(&mut self, a: Point, b: Point, color: Color, cap: LineCap, thickness: f32) {
      assert!(
         self.line_data.len() < self.line_data.capacity(),
         "too many lines without flushing"
      );

      self.line_data.push(LineData {
         line: vec4(a.x, a.y, b.x, b.y),
         thickness,
         cap: match cap {
            LineCap::Butt => LineData::BUTT,
            LineCap::Square => LineData::SQUARE,
            LineCap::Round => LineData::ROUND,
         },
         color,
      });
   }

   pub fn flush<'a>(
      &'a mut self,
      context: &mut FlushContext<'a>,
      render_pass: &mut wgpu::RenderPass<'a>,
   ) {
      // TODO: This should interact with clearing, probably.
      if self.line_data.is_empty() {
         return;
      }

      let (line_data_buffer, bind_group) = self.batch_storage.next_batch(context.gpu);

      let line_data_bytes = bytemuck::cast_slice(&self.line_data);
      context.gpu.queue.write_buffer(line_data_buffer, 0, line_data_bytes);

      render_pass.push_debug_group("Lines");
      render_pass.set_pipeline(&self.render_pipelines.get(context.blend_mode));
      render_pass.set_bind_group(0, bind_group, &[]);
      render_pass.set_bind_group(1, context.model_transform_bind_group, &[]);
      render_pass.set_bind_group(2, &context.scene_uniform_bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..6, 0..self.line_data.len() as u32);
      render_pass.pop_debug_group();

      self.line_data.clear();
   }

   pub fn rewind(&mut self) {
      self.batch_storage.rewind();
   }

   pub fn needs_flush(&self) -> bool {
      self.line_data.len() == self.line_data.capacity()
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct LineData {
   line: Vec4,
   thickness: f32,
   cap: u32,
   color: Color,
}

unsafe impl Pod for LineData {}
unsafe impl Zeroable for LineData {}

impl LineData {
   const BUTT: u32 = 0;
   const SQUARE: u32 = 1;
   const ROUND: u32 = 2;
}
