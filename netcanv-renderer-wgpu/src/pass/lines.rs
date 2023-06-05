use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{vec4, Vec4};
use netcanv_renderer::paws::{Color, LineCap, Point};
use wgpu::util::DeviceExt;

use crate::batch_storage::{BatchStorage, BatchStorageConfig};
use crate::gpu::Gpu;
use crate::ClearOps;

use super::vertex::{vertex, Vertex};

pub(crate) struct Lines {
   vertex_buffer: wgpu::Buffer,
   batch_storage: BatchStorage,
   render_pipeline: wgpu::RenderPipeline,

   line_data: Vec<LineData>,
}

impl Lines {
   pub const RESERVED_LINE_COUNT: usize = 512;

   pub fn new(gpu: &Gpu) -> Self {
      let shader = gpu.device.create_shader_module(wgpu::include_wgsl!("shader/lines.wgsl"));

      let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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

      let (scene_uniforms_layout, _) = gpu.scene_uniforms_binding(0);
      let bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Lines: Bind Group Layout"),
            entries: &[
               scene_uniforms_layout,
               wgpu::BindGroupLayoutEntry {
                  binding: 1,
                  visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                  ty: wgpu::BindingType::Buffer {
                     ty: wgpu::BufferBindingType::Uniform,
                     has_dynamic_offset: false,
                     min_binding_size: None,
                  },
                  count: None,
               },
            ],
         });

      let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
         label: Some("Lines: Render Pipeline Layout"),
         bind_group_layouts: &[&bind_group_layout],
         push_constant_ranges: &[],
      });
      let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
         label: Some("Lines: Render Pipeline"),
         layout: Some(&pipeline_layout),
         vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "main_vs",
            buffers: &[Vertex::LAYOUT],
         },
         primitive: wgpu::PrimitiveState::default(),
         depth_stencil: Some(gpu.depth_stencil_state()),
         multisample: wgpu::MultisampleState::default(),
         fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "main_fs",
            targets: &[Some(gpu.color_target_state())],
         }),
         multiview: None,
      });

      Self {
         vertex_buffer,
         batch_storage: BatchStorage::new(BatchStorageConfig {
            buffer_name: "Lines: Line Data Buffer",
            bind_group_name: "Lines: Bind Group",
            buffer_size: (size_of::<LineData>() * Self::RESERVED_LINE_COUNT) as wgpu::BufferAddress,
            bind_group_layout,
         }),
         render_pipeline,
         line_data: Vec::with_capacity(Self::RESERVED_LINE_COUNT),
      }
   }

   pub fn add(
      &mut self,
      depth_index: u32,
      a: Point,
      b: Point,
      color: Color,
      cap: LineCap,
      thickness: f32,
   ) {
      assert!(
         self.line_data.len() < self.line_data.capacity(),
         "too many lines without flushing"
      );

      self.line_data.push(LineData {
         line: vec4(a.x, a.y, b.x, b.y),
         depth_index,
         thickness,
         cap: match cap {
            LineCap::Butt => LineData::BUTT,
            LineCap::Square => LineData::SQUARE,
            LineCap::Round => LineData::ROUND,
         },
         color,
      });
   }

   pub fn flush(
      &mut self,
      gpu: &Gpu,
      encoder: &mut wgpu::CommandEncoder,
      clear_ops: &mut ClearOps,
   ) {
      // TODO: This should interact with clearing, probably.
      if self.line_data.is_empty() {
         return;
      }

      let (line_data_buffer, bind_group) = self.batch_storage.next_batch(gpu);

      let line_data_bytes = bytemuck::cast_slice(&self.line_data);
      gpu.queue.write_buffer(line_data_buffer, 0, line_data_bytes);

      let ClearOps { color, depth } = clear_ops.take();
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
         label: Some("Lines"),
         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: gpu.render_target(),
            resolve_target: None,
            ops: color,
         })],
         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &gpu.depth_buffer_view,
            depth_ops: Some(depth),
            stencil_ops: None,
         }),
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..6, 0..self.line_data.len() as u32);

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
   depth_index: u32,
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
