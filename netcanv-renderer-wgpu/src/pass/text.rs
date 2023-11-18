use std::mem::size_of;
use std::ops::Range;

use bytemuck::{Pod, Zeroable};
use glam::Vec2;
use netcanv_renderer::paws::Color;
use tracing::warn;
use wgpu::util::DeviceExt;

use crate::batch_storage::{BatchStorage, BatchStorageConfig};
use crate::rendering::{BlendFlags, FlushContext};
use crate::text::TextRenderer;
use crate::Font;

use super::vertex::{vertex, Vertex};
use super::{PassCreationContext, RenderPipelinePermutations};

pub(crate) struct Text {
   vertex_buffer: wgpu::Buffer,
   batch_storage: BatchStorage,
   render_pipelines: RenderPipelinePermutations,

   glyph_data: Vec<GlyphData>,
   font_spans: Vec<FontSpan>,
}

#[derive(Debug, Clone)]
struct FontSpan {
   range: Range<u32>,
   font_key: (swash::CacheKey, u32),
}

impl Text {
   const BUFFER_GLYPH_COUNT: usize = 1024;

   pub fn new(context: &PassCreationContext<'_>, text_renderer: &TextRenderer) -> Self {
      let shader = context.gpu.device.create_shader_module(wgpu::include_wgsl!("shader/text.wgsl"));

      let vertex_buffer =
         context.gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Text: Vertex Buffer"),
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

      let glyph_data_bind_group_layout =
         context.gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text: Data Buffer Bind Group Layout"),
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
            label: Some("Text: Render Pipeline Layout"),
            bind_group_layouts: &[
               &glyph_data_bind_group_layout,
               &text_renderer.font_bind_group_layout,
               context.model_transform_bind_group_layout,
               &context.gpu.scene_uniform_bind_group_layout,
            ],
            push_constant_ranges: &[],
         });
      let render_pipelines = RenderPipelinePermutations::new(|label, blend_mode| {
         context.gpu.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(&format!("Text: Render Pipeline {label}")),
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
            name: "Text",
            buffer_size: (Self::BUFFER_GLYPH_COUNT * size_of::<GlyphData>()) as wgpu::BufferAddress,
            bind_group_layout: glyph_data_bind_group_layout,
         }),
         render_pipelines,
         glyph_data: Vec::with_capacity(1024),
         font_spans: vec![],
      }
   }

   pub fn glyph_index(&self) -> u32 {
      self.glyph_data.len() as u32
   }

   pub fn add_glyph(&mut self, position: Vec2, glyph: u32, color: Color, blend_flags: BlendFlags) {
      self.glyph_data.push(GlyphData {
         position,
         rendition: blend_flags.bits() << 30 | glyph,
         color,
      });
   }

   pub fn add_font_span(&mut self, range: Range<u32>, font: &Font) {
      let key = font.key();
      if let Some(last_span) = self.font_spans.last_mut() {
         if last_span.font_key == key && last_span.range.end == range.start {
            last_span.range.end = range.end;
            return;
         }
      }
      self.font_spans.push(FontSpan {
         range,
         font_key: key,
      });
   }

   pub fn rewind(&mut self) {
      self.batch_storage.rewind();
   }

   pub fn flush<'a>(
      &'a mut self,
      context: &mut FlushContext<'a>,
      text_renderer: &'a mut TextRenderer,
      render_pass: &mut wgpu::RenderPass<'a>,
   ) {
      if self.glyph_data.is_empty() {
         return;
      }

      render_pass.push_debug_group("Text");
      render_pass.set_pipeline(self.render_pipelines.get(context.blend_mode));
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.set_bind_group(2, context.model_transform_bind_group, &[]);
      render_pass.set_bind_group(3, context.scene_uniform_bind_group, &[]);

      // Need to reserve batches up front here, as adding a batch can potentially reallocate the
      // batch buffers and that's not something the borrow checker likes. And neither do I, tbh.
      let batch_count: usize = self
         .font_spans
         .iter()
         .map(|span| {
            (span.range.end - span.range.start + Self::BUFFER_GLYPH_COUNT as u32 - 1)
               / Self::BUFFER_GLYPH_COUNT as u32
         })
         .map(|count| count as usize)
         .sum();
      let mut batches = self.batch_storage.next_many(context.gpu, batch_count);

      for span in &self.font_spans {
         if let Some(gpu_font) = text_renderer.fonts.get(&span.font_key) {
            render_pass.set_bind_group(1, &gpu_font.bind_group, &[]);
            for start in span.range.clone().step_by(Self::BUFFER_GLYPH_COUNT) {
               let end = (start + Self::BUFFER_GLYPH_COUNT as u32).min(span.range.end);
               let (buffer, bind_group) = batches.next().unwrap();
               let chunk = &self.glyph_data[start as usize..end as usize];
               context.gpu.queue.write_buffer(buffer, 0, bytemuck::cast_slice(chunk));
               render_pass.set_bind_group(0, bind_group, &[]);
               render_pass.draw(0..6, 0..chunk.len() as u32);
            }
         } else {
            warn!("{span:?} has invalid font key");
         }
      }

      render_pass.pop_debug_group();

      self.glyph_data.clear();
      self.font_spans.clear();
   }
}

#[derive(Debug, Clone, Copy)]
#[repr(C, align(16))]
struct GlyphData {
   position: Vec2,
   rendition: u32,
   color: Color,
}

unsafe impl Zeroable for GlyphData {}
unsafe impl Pod for GlyphData {}
