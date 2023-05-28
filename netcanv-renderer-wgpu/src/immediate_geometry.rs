use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use wgpu::util::DeviceExt;
use wgpu::TextureView;

use crate::gpu::Gpu;

/// Pipeline for drawing an immediate vertex buffer.
pub struct ImmediateGeometry {
   vertex_buffer: wgpu::Buffer,
   bind_group: wgpu::BindGroup,
   render_pipeline: wgpu::RenderPipeline,
}

#[derive(Debug, Clone, Copy)]
pub struct Vertex {
   pub position: Vec2,
}

unsafe impl Zeroable for Vertex {}
unsafe impl Pod for Vertex {}

impl ImmediateGeometry {
   pub fn new(gpu: &Gpu) -> Self {
      let swapchain_capabilities = gpu.surface.get_capabilities(&gpu.adapter);
      let swapchain_format = swapchain_capabilities.formats[0];

      let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
         label: Some("Immediate Geometry Vertex Buffer"),
         contents: bytemuck::cast_slice(&[0.0_f32, 0.0, 32.0, 0.0, 0.0, 32.0]),
         usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
      });

      let bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Immediate Geometry Bind Group Layout"),
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
         label: Some("Immediate Geometry Pipeline Layout"),
         bind_group_layouts: &[&bind_group_layout],
         push_constant_ranges: &[],
      });
      let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
         label: Some("Immediate Geometry Bind Group"),
         layout: &bind_group_layout,
         entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: gpu.uniform_buffer.as_entire_binding(),
         }],
      });

      let shader = gpu.device.create_shader_module(wgpu::ShaderModuleDescriptor {
         label: Some("Immediate Geometry Shader"),
         source: wgpu::ShaderSource::Wgsl(include_str!("shader/solid.wgsl").into()),
      });

      let render_pipeline = gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
         label: Some("Immediate Geometry"),
         layout: Some(&pipeline_layout),
         vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "main_vs",
            buffers: &[wgpu::VertexBufferLayout {
               array_stride: (std::mem::size_of::<f32>() * 2) as wgpu::BufferAddress,
               step_mode: wgpu::VertexStepMode::Vertex,
               attributes: &[wgpu::VertexAttribute {
                  format: wgpu::VertexFormat::Float32x2,
                  offset: 0,
                  shader_location: 0,
               }],
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
      }
   }

   pub fn draw(
      &self,
      what: &str,
      gpu: &Gpu,
      encoder: &mut wgpu::CommandEncoder,
      render_target: &TextureView,
      vertices: &[Vertex],
   ) {
      gpu.queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
         label: Some(what),
         color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: render_target,
            resolve_target: None,
            ops: wgpu::Operations {
               load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
               store: true,
            },
         })],
         depth_stencil_attachment: None,
      });
      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.bind_group, &[]);
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..3, 0..1);
   }
}
