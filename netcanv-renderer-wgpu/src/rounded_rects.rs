use std::mem::size_of;

use bytemuck::{offset_of, Pod, Zeroable};
use glam::{vec4, Vec2, Vec4, vec2};
use netcanv_renderer::paws::{Color, Rect};
use wgpu::include_wgsl;
use wgpu::util::DeviceExt;

use crate::gpu::Gpu;
use crate::ClearOps;

/// Pipeline for drawing rounded rectangles.
pub struct RoundedRects {
   shader: wgpu::ShaderModule,
   vertex_buffer: wgpu::Buffer,
   rect_data_buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   render_pipeline: wgpu::RenderPipeline,

   rect_data: Vec<RectData>,
}

impl RoundedRects {
   const RESERVED_RECT_COUNT: u32 = 256;

   pub fn new(gpu: &Gpu) -> Self {
      let texture_format = gpu.surface_format();

      let shader = gpu.device.create_shader_module(include_wgsl!("shader/rounded_rects.wgsl"));

      let vertex_buffer =
         gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("RoundedRects: Vertex Buffer"),
            contents: bytemuck::cast_slice(&[
               vertex(1.0, 1.0),
               vertex(0.0, 1.0),
               vertex(0.0, 0.0),
               vertex(1.0, 1.0),
               vertex(1.0, 0.0),
               vertex(0.0, 0.0),
            ]),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
         });
      let rect_data_buffer = Self::create_rect_data_buffer(gpu, Self::RESERVED_RECT_COUNT as usize);

      let (bind_group, render_pipeline) =
         Self::create_pipeline(gpu, &shader, texture_format, &rect_data_buffer);

      Self {
         shader,
         vertex_buffer,
         rect_data_buffer,
         bind_group,
         render_pipeline,
         rect_data: Vec::with_capacity(Self::RESERVED_RECT_COUNT as usize),
      }
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
         depth_stencil: Some(wgpu::DepthStencilState {
            format: gpu.depth_buffer.format(),
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
         }),
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

   pub fn add(
      &mut self,
      depth_index: u32,
      rect: Rect,
      color: Color,
      corner_radius: f32,
      outline: f32,
   ) {
      assert!(
         self.rect_data.len() <= self.rect_data.capacity(),
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
      let rect_data_bytes: &[u8] = bytemuck::cast_slice(&self.rect_data);
      if rect_data_bytes.len() as wgpu::BufferAddress > self.rect_data_buffer.size() {
         self.rect_data_buffer.destroy();
         self.rect_data_buffer = Self::create_rect_data_buffer(gpu, self.rect_data.len());
         self.update_pipeline(gpu);
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

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
struct Vertex {
   position: Vec2,
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}

fn vertex(x: f32, y: f32) -> Vertex {
   Vertex {
      position: vec2(x, y),
   }
}
