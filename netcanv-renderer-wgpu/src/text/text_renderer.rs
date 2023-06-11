use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use glam::{vec2, Vec2};
use swash::scale::{Render, Source, StrikeWith};

use crate::gpu::Gpu;
use crate::Font;

use super::caches::Caches;
use super::gpu_font::GpuFont;

pub struct TextRenderer {
   pub caches: Rc<RefCell<Caches>>,
   pub font_bind_group_layout: wgpu::BindGroupLayout,
   pub fonts: HashMap<(swash::CacheKey, u32), GpuFont>,
}

impl TextRenderer {
   pub fn new(gpu: &Gpu) -> Self {
      let bitmap_bind_group_layout =
         gpu.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Font: Bind Group Layout"),
            entries: &[
               wgpu::BindGroupLayoutEntry {
                  binding: 0,
                  visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                  ty: wgpu::BindingType::Texture {
                     sample_type: wgpu::TextureSampleType::Float { filterable: true },
                     view_dimension: wgpu::TextureViewDimension::D2,
                     multisampled: false,
                  },
                  count: None,
               },
               wgpu::BindGroupLayoutEntry {
                  binding: 1,
                  visibility: wgpu::ShaderStages::VERTEX,
                  ty: wgpu::BindingType::Buffer {
                     ty: wgpu::BufferBindingType::Uniform,
                     has_dynamic_offset: false,
                     min_binding_size: None,
                  },
                  count: None,
               },
            ],
         });
      Self {
         caches: Rc::new(RefCell::new(Caches::new())),
         font_bind_group_layout: bitmap_bind_group_layout,
         fonts: HashMap::new(),
      }
   }
}

impl TextRenderer {
   /// Shapes and renders glyphs into a texture. f receives each glyph's position and in-GPU
   /// glyph index.
   pub fn render_text(
      &mut self,
      gpu: &Gpu,
      font: &Font,
      text: &str,
      origin: Vec2,
      mut next_glyph: impl FnMut(Vec2, u32),
   ) {
      let mut caches = font.caches.borrow_mut();
      let caches = &mut *caches; // Deref needed because you can't borrow individual fields out of one.
      let mut shaper =
         caches.shape_context.builder(font.data.as_font_ref()).size(font.normalized_size()).build();
      let mut scaler = caches
         .scale_context
         .builder(font.data.as_font_ref())
         .hint(true)
         .size(font.normalized_size())
         .build();
      shaper.add_str(text);

      let gpu_font = self.fonts.entry(font.key()).or_insert_with(|| {
         GpuFont::new(
            gpu,
            &self.font_bind_group_layout,
            &format!(
               "Font #{} @ {}ppem",
               font.data.key.value(),
               font.normalized_size()
            ),
         )
      });

      let mut pen = origin;
      shaper.shape_with(|glyph_cluster| {
         for glyph in glyph_cluster.glyphs {
            const X_SUBPOSITIONS: f32 = 8.0;
            let x_subposition = (pen.x.fract() * X_SUBPOSITIONS) as u8;
            let x_offset = x_subposition as f32 / X_SUBPOSITIONS;

            if let Some((gpu_index, placement)) =
               gpu_font.get_or_upload_glyph(gpu, glyph.id, x_subposition, || {
                  let image = Render::new(&[Source::Outline, Source::Bitmap(StrikeWith::BestFit)])
                     .offset(swash::zeno::Vector {
                        x: x_offset,
                        y: 0.0,
                     })
                     .render(&mut scaler, glyph.id);
                  image.map(|image| (image.placement, image.data))
               })
            {
               next_glyph(
                  pen + vec2(placement.left as f32, -placement.top as f32),
                  gpu_index,
               );
            }

            pen.x += glyph.advance;
         }
      });
   }
}
