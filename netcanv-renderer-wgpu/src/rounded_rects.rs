use std::mem::size_of;

use bytemuck::{offset_of, Pod, Zeroable};
use glam::{vec4, Vec2, Vec4};
use netcanv_renderer::paws::{Color, Rect};
use wgpu::include_wgsl;

use crate::common::vector_to_vec2;
use crate::gpu::Gpu;

/// Pipeline for drawing rounded rectangles.
pub struct RoundedRects {
   shader: wgpu::ShaderModule,
   vertex_buffer: wgpu::Buffer,
   rect_data_buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   render_pipeline: wgpu::RenderPipeline,

   vertices: Vec<Vertex>,
   rect_data: Vec<RectData>,
}

impl RoundedRects {
   const RESERVED_RECT_COUNT: u32 = 256;
   const RESERVED_VERTEX_COUNT: usize = Self::RESERVED_RECT_COUNT as usize * 6;

   pub fn new(gpu: &Gpu) -> Self {
      let texture_format = gpu.surface_format();

      let shader = gpu.device.create_shader_module(include_wgsl!("shader/rounded_rects.wgsl"));

      let vertex_buffer = Self::create_vertex_buffer(gpu, Self::RESERVED_VERTEX_COUNT);
      let rect_data_buffer = Self::create_rect_data_buffer(gpu, Self::RESERVED_RECT_COUNT as usize);

      let (bind_group, render_pipeline) =
         Self::create_pipeline(gpu, &shader, texture_format, &rect_data_buffer);

      Self {
         shader,
         vertex_buffer,
         rect_data_buffer,
         bind_group,
         render_pipeline,
         vertices: Vec::with_capacity(Self::RESERVED_VERTEX_COUNT),
         rect_data: Vec::with_capacity(Self::RESERVED_RECT_COUNT as usize),
      }
   }

   fn create_vertex_buffer(gpu: &Gpu, vertex_count: usize) -> wgpu::Buffer {
      gpu.device.create_buffer(&wgpu::BufferDescriptor {
         label: Some("RoundedRects: Vertex Buffer"),
         size: (size_of::<Vertex>() * vertex_count) as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
         mapped_at_creation: false,
      })
   }

   fn create_rect_data_buffer(gpu: &Gpu, rect_count: usize) -> wgpu::Buffer {
      gpu.device.create_buffer(&wgpu::BufferDescriptor {
         label: Some("RoundedRects: Data Buffer"),
         size: (size_of::<RectData>() * rect_count) as wgpu::BufferAddress,
         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
         mapped_at_creation: false,
      })
   }

   fn create_pipeline(
      gpu: &Gpu,
      shader: &wgpu::ShaderModule,
      texture_format: wgpu::TextureFormat,
      rect_data_buffer: &wgpu::Buffer,
   ) -> (wgpu::BindGroup, wgpu::RenderPipeline) {
      let bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("RoundedRects: Bind Group Layout"),
            entries: &[
               wgpu::BindGroupLayoutEntry {
                  binding: 0,
                  visibility: wgpu::ShaderStages::VERTEX,
                  ty: wgpu::BindingType::Buffer {
                     ty: wgpu::BufferBindingType::Uniform,
                     has_dynamic_offset: false,
                     min_binding_size: None,
                  },
                  count: None,
               },
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
            wgpu::BindGroupEntry {
               binding: 0,
               resource: gpu.scene_uniform_buffer.as_entire_binding(),
            },
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
            buffers: &[wgpu::VertexBufferLayout {
               array_stride: size_of::<Vertex>() as wgpu::BufferAddress,
               step_mode: wgpu::VertexStepMode::Vertex,
               attributes: &[
                  wgpu::VertexAttribute {
                     format: wgpu::VertexFormat::Float32x2,
                     offset: offset_of!(Vertex, position) as wgpu::BufferAddress,
                     shader_location: 0,
                  },
                  wgpu::VertexAttribute {
                     format: wgpu::VertexFormat::Uint32,
                     offset: offset_of!(Vertex, rect_index) as wgpu::BufferAddress,
                     shader_location: 1,
                  },
               ],
            }],
         },
         primitive: wgpu::PrimitiveState::default(),
         fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: "main_fs",
            targets: &[Some({
               wgpu::ColorTargetState {
                  format: texture_format,
                  blend: Some(wgpu::BlendState {
                     color: wgpu::BlendComponent::OVER,
                     alpha: wgpu::BlendComponent::OVER,
                  }),
                  write_mask: wgpu::ColorWrites::ALL,
               }
            })],
         }),
         depth_stencil: None,
         multisample: wgpu::MultisampleState::default(),
         multiview: None,
      });
      (bind_group, render_pipeline)
   }

   fn update_pipeline(&mut self, gpu: &Gpu) {
      let texture_format = gpu.surface_format();
      (self.bind_group, self.render_pipeline) =
         Self::create_pipeline(gpu, &self.shader, texture_format, &self.rect_data_buffer);
   }

   pub fn add(&mut self, depth_index: u32, rect: Rect, color: Color, corner_radius: f32) {
      assert!(
         self.rect_data.len() <= self.rect_data.capacity(),
         "too many rectangles without flushing"
      );

      let rect_index = self.rect_data.len() as u32;
      self.rect_data.push(RectData {
         color,
         depth_index,
         rect: vec4(rect.left(), rect.top(), rect.right(), rect.bottom()),
         corner_radius,
      });
      self.vertices.extend_from_slice(&[
         vertex(rect_index, vector_to_vec2(rect.top_right())),
         vertex(rect_index, vector_to_vec2(rect.top_left())),
         vertex(rect_index, vector_to_vec2(rect.bottom_left())),
         vertex(rect_index, vector_to_vec2(rect.bottom_left())),
         vertex(rect_index, vector_to_vec2(rect.bottom_right())),
         vertex(rect_index, vector_to_vec2(rect.top_right())),
      ]);
   }

   pub fn flush(
      &mut self,
      gpu: &Gpu,
      encoder: &mut wgpu::CommandEncoder,
      ops: wgpu::Operations<wgpu::Color>,
   ) {
      let vertex_bytes = bytemuck::cast_slice(&self.vertices);
      if vertex_bytes.len() as wgpu::BufferAddress > self.vertex_buffer.size() {
         self.vertex_buffer.destroy();
         self.vertex_buffer = Self::create_vertex_buffer(gpu, self.vertices.len());
      }
      gpu.queue.write_buffer(&self.vertex_buffer, 0, vertex_bytes);

      let rect_data_bytes: &[u8] = bytemuck::cast_slice(&self.rect_data);
      if rect_data_bytes.len() as wgpu::BufferAddress > self.rect_data_buffer.size() {
         self.rect_data_buffer.destroy();
         self.rect_data_buffer = Self::create_rect_data_buffer(gpu, self.rect_data.len());
         self.update_pipeline(gpu);
      }
      let rect_data_bytes = bytemuck::cast_slice(&self.rect_data);
      gpu.queue.write_buffer(&self.rect_data_buffer, 0, rect_data_bytes);

      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
         label: Some("RoundedRects"),
         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: gpu.render_target(),
            resolve_target: None,
            ops,
         })],
         depth_stencil_attachment: None,
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..self.vertices.len() as u32, 0..1);

      self.vertices.clear();
      self.rect_data.clear();
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct RectData {
   rect: Vec4,
   depth_index: u32,
   corner_radius: f32,
   color: Color,
}

impl Default for RectData {
   fn default() -> Self {
      Self {
         rect: Default::default(),
         depth_index: Default::default(),
         corner_radius: Default::default(),
         color: Color::TRANSPARENT,
      }
   }
}

unsafe impl Zeroable for RectData {}
unsafe impl Pod for RectData {}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct Vertex {
   position: Vec2,
   rect_index: u32,
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}

fn vertex(rect_index: u32, position: Vec2) -> Vertex {
   Vertex {
      position,
      rect_index,
   }
}
