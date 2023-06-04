use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use glam::{vec4, Vec4};
use netcanv_renderer::paws::{Color, Rect};
use wgpu::include_wgsl;
use wgpu::util::DeviceExt;

use crate::gpu::Gpu;
use crate::ClearOps;

use super::vertex::{vertex, Vertex};

/// Pipeline for drawing rounded rectangles.
pub struct RoundedRects {
   vertex_buffer: wgpu::Buffer,
   rect_data_buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   render_pipeline: wgpu::RenderPipeline,

   rect_data: Vec<RectData>,
}

impl RoundedRects {
   const RESERVED_RECT_COUNT: usize = 256;

   pub fn new(gpu: &Gpu) -> Self {
      let shader = gpu.device.create_shader_module(include_wgsl!("shader/rounded_rects.wgsl"));

      let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
      let rect_data_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
         label: Some("RoundedRects: Data Buffer"),
         size: (size_of::<RectData>() * Self::RESERVED_RECT_COUNT) as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
         mapped_at_creation: false,
      });

      let (bind_group, render_pipeline) = Self::create_pipeline(gpu, &shader, &rect_data_buffer);

      Self {
         vertex_buffer,
         rect_data_buffer,
         bind_group,
         render_pipeline,
         rect_data: Vec::with_capacity(Self::RESERVED_RECT_COUNT),
      }
   }

   fn create_pipeline(
      gpu: &Gpu,
      shader: &wgpu::ShaderModule,
      rect_data_buffer: &wgpu::Buffer,
   ) -> (wgpu::BindGroup, wgpu::RenderPipeline) {
      let (scene_uniforms_layout, scene_uniforms) = gpu.scene_uniforms_binding(0);

      let bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RoundedRects: Bind Group Layout"),
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
         label: Some("RoundedRects: Pipeline Layout"),
         bind_group_layouts: &[&bind_group_layout],
         push_constant_ranges: &[],
      });
      let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some("RoundedRects: Bind Group"),
         layout: &bind_group_layout,
         entries: &[
            scene_uniforms,
            wgpu::BindGroupEntry {
               binding: 1,
               resource: rect_data_buffer.as_entire_binding(),
            },
         ],
      });

      let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
         label: Some("RoundedRects: Render Pipeline"),
         layout: Some(&pipeline_layout),
         vertex: wgpu::VertexState {
            module: shader,
            entry_point: "main_vs",
            buffers: &[Vertex::LAYOUT],
         },
         primitive: wgpu::PrimitiveState::default(),
         fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "main_fs",
            targets: &[Some(gpu.color_target_state())],
         }),
         depth_stencil: Some(gpu.depth_stencil_state()),
         multisample: wgpu::MultisampleState::default(),
         multiview: None,
      });
      (bind_group, render_pipeline)
   }

   pub fn add(
      &mut self,
      depth_index: u32,
      rect: Rect,
      color: Color,
      corner_radius: f32,
      outline: f32,
   ) {
      assert!(
         self.rect_data.len() < self.rect_data.capacity(),
         "too many rectangles without flushing"
      );

      self.rect_data.push(RectData {
         color,
         depth_index,
         rect: vec4(rect.left(), rect.top(), rect.width(), rect.height()),
         corner_radius,
         outline,
      });
   }

   pub fn flush(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder, clear_ops: ClearOps) {
      // TODO: This should interact with clearing, probably.
      if self.rect_data.is_empty() {
         return;
      }

      let rect_data_bytes = bytemuck::cast_slice(&self.rect_data);
      gpu.queue.write_buffer(&self.rect_data_buffer, 0, rect_data_bytes);

      let (color_ops, depth_ops) = clear_ops;
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
         label: Some("RoundedRects"),
         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: gpu.render_target(),
            resolve_target: None,
            ops: color_ops,
         })],
         depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &gpu.depth_buffer_view,
            depth_ops: Some(depth_ops),
            stencil_ops: None,
         }),
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..6, 0..self.rect_data.len() as u32);

      self.rect_data.clear();
   }

   pub fn needs_flush(&self) -> bool {
      self.rect_data.len() == self.rect_data.capacity()
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct RectData {
   rect: Vec4,
   depth_index: u32,
   corner_radius: f32,
   color: Color,
   /// This being <= 0 means we should fill in the rectangle.
   outline: f32,
}

impl Default for RectData {
   fn default() -> Self {
      Self {
         rect: Default::default(),
         depth_index: Default::default(),
         corner_radius: Default::default(),
         color: Color::TRANSPARENT,
         outline: 0.0,
      }
   }
}

unsafe impl Zeroable for RectData {}
unsafe impl Pod for RectData {}
