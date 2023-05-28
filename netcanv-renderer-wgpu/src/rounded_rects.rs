use bytemuck::{offset_of, Pod, Zeroable};
use glam::Vec2;
use netcanv_renderer::paws::{Color, Rect};

use crate::common::vector_to_vec2;
use crate::gpu::Gpu;

/// Pipeline for drawing rounded rectangles.
pub struct RoundedRects {
   vertex_buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   render_pipeline: wgpu::RenderPipeline,

   vertices: Vec<Vertex>,
}

impl RoundedRects {
   const RESERVED_VERTEX_COUNT: usize = 1024;

   pub fn new(gpu: &Gpu) -> Self {
      let swapchain_capabilities = gpu.surface.get_capabilities(&gpu.adapter);
      let swapchain_format = swapchain_capabilities.formats[0];

      let vertex_buffer = create_vertex_buffer(gpu, Self::RESERVED_VERTEX_COUNT);

      let bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Rounded Rectangles Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
               binding: 0,
               visibility: wgpu::ShaderStages::VERTEX,
               ty: wgpu::BindingType::Buffer {
                  ty: wgpu::BufferBindingType::Uniform,
                  has_dynamic_offset: false,
                  min_binding_size: None,
               },
               count: None,
            }],
         });
      let pipeline_layout = gpu.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
         label: Some("Rounded Rectangles Pipeline Layout"),
         bind_group_layouts: &[&bind_group_layout],
         push_constant_ranges: &[],
      });
      let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some("Rounded Rectangles Bind Group"),
         layout: &bind_group_layout,
         entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: gpu.scene_uniform_buffer.as_entire_binding(),
         }],
      });

      let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
         label: Some("Rounded Rectangles Shader"),
         source: wgpu::ShaderSource::Wgsl(include_str!("shader/rounded_rects.wgsl").into()),
      });

      let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
         label: Some("Rounded Rectangles"),
         layout: Some(&pipeline_layout),
         vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "main_vs",
            buffers: &[wgpu::VertexBufferLayout {
               array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
               step_mode: wgpu::VertexStepMode::Vertex,
               attributes: &[
                  wgpu::VertexAttribute {
                     format: wgpu::VertexFormat::Float32x2,
                     offset: offset_of!(Vertex, position) as wgpu::BufferAddress,
                     shader_location: 0,
                  },
                  wgpu::VertexAttribute {
                     format: wgpu::VertexFormat::Uint32,
                     offset: offset_of!(Vertex, depth_index) as wgpu::BufferAddress,
                     shader_location: 1,
                  },
                  wgpu::VertexAttribute {
                     format: wgpu::VertexFormat::Uint8x4,
                     offset: offset_of!(Vertex, color) as wgpu::BufferAddress,
                     shader_location: 2,
                  },
               ],
            }],
         },
         primitive: wgpu::PrimitiveState::default(),
         fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "main_fs",
            targets: &[Some(swapchain_format.into())],
         }),
         depth_stencil: None,
         multisample: wgpu::MultisampleState::default(),
         multiview: None,
      });

      Self {
         vertex_buffer,
         bind_group,
         render_pipeline,
         vertices: Vec::with_capacity(Self::RESERVED_VERTEX_COUNT),
      }
   }

   pub fn add(&mut self, depth_index: u32, rect: Rect, color: Color) {
      self.vertices.extend_from_slice(&[
         vertex(depth_index, color, vector_to_vec2(rect.top_right())),
         vertex(depth_index, color, vector_to_vec2(rect.top_left())),
         vertex(depth_index, color, vector_to_vec2(rect.bottom_left())),
         vertex(depth_index, color, vector_to_vec2(rect.bottom_left())),
         vertex(depth_index, color, vector_to_vec2(rect.bottom_right())),
         vertex(depth_index, color, vector_to_vec2(rect.top_right())),
      ]);
   }

   pub fn flush(&mut self, gpu: &Gpu, encoder: &mut wgpu::CommandEncoder) {
      let bytes = bytemuck::cast_slice(&self.vertices);
      if bytes.len() as wgpu::BufferAddress > self.vertex_buffer.size() {
         self.vertex_buffer.destroy();
         self.vertex_buffer = create_vertex_buffer(gpu, self.vertices.len());
      } else {
         gpu.queue.write_buffer(&self.vertex_buffer, 0, bytes);
      }

      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
         label: Some("Draw Rounded Rects"),
         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: gpu.render_target(),
            resolve_target: None,
            ops: wgpu::Operations {
               load: wgpu::LoadOp::Load,
               store: true,
            },
         })],
         depth_stencil_attachment: None,
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..self.vertices.len() as u32, 0..1);

      self.vertices.clear();
   }
}

fn create_vertex_buffer(gpu: &Gpu, vertex_count: usize) -> wgpu::Buffer {
   gpu.device.create_buffer(&wgpu::BufferDescriptor {
      label: Some("Rounded Rectangles Vertex Buffer"),
      size: (std::mem::size_of::<Vertex>() * vertex_count) as wgpu::BufferAddress,
      usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
      mapped_at_creation: false,
   })
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct Vertex {
   position: Vec2,
   color: u32,
   depth_index: u32,
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}

fn vertex(depth_index: u32, color: Color, position: Vec2) -> Vertex {
   Vertex {
      position,
      color: color.to_argb(),
      depth_index,
   }
}
