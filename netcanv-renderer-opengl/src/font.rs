//! A <del>quite shitty</del> text renderer based on swash.
//!
//! Does not support advanced features such as shaping, or text wrapping.

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::Chars;

use glow::{HasContext, PixelUnpackData};
use netcanv_renderer::paws::{vector, Rect, Vector};
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::shape::ShapeContext;
use swash::zeno::Format;
use swash::{CacheKey, FontRef};

use crate::common::{GlUtilities, RectMath};
use crate::rect_packer::RectPacker;

const TEXTURE_ATLAS_SIZE: u32 = 1024;

struct Glyph {
   uv_rect: Rect,
   size: Vector,
   offset: Vector,
   advance_x: f32,
}

struct FontSize {
   size: u32,
   texture: glow::Texture,
   packer: RectPacker,
   ascii: [Option<Glyph>; 256],
   unicode: HashMap<char, Glyph>,
   height: f32,
}

impl FontSize {
   fn insert_glyph(&mut self, c: char, glyph: Glyph) {
      let character_index = c as usize;
      if character_index <= 255 {
         self.ascii[character_index] = Some(glyph);
      } else {
         self.unicode.insert(c, glyph);
      }
   }

   fn get_glyph(&self, c: char) -> Option<&Glyph> {
      let character_index = c as usize;
      if character_index <= 255 {
         self.ascii[character_index].as_ref()
      } else {
         self.unicode.get(&c)
      }
   }
}

struct SwashFont {
   data: Vec<u8>,
   offset: u32,
   key: CacheKey,
}

impl SwashFont {
   fn new(data: Vec<u8>) -> Option<Self> {
      let font = FontRef::from_index(&data, 0)?;
      let (offset, key) = (font.offset, font.key);

      Some(Self { data, offset, key })
   }

   fn as_ref(&self) -> FontRef {
      FontRef {
         data: &self.data,
         offset: self.offset,
         key: self.key,
      }
   }
}

struct FontFace {
   gl: Rc<glow::Context>,
   swash_font: SwashFont,
   sizes: HashMap<u32, FontSize>,
   shape_context: ShapeContext,
   scale_context: ScaleContext,
}

impl FontFace {
   fn new(gl: Rc<glow::Context>, data: Vec<u8>) -> Option<Self> {
      let face = SwashFont::new(data)?;

      Some(Self {
         gl,
         swash_font: face,
         sizes: HashMap::new(),
         shape_context: ShapeContext::new(),
         scale_context: ScaleContext::new(),
      })
   }

   fn make_size(&mut self, size: u32) {
      if self.sizes.contains_key(&size) {
         return;
      }
      let gl = &self.gl;
      let swash_font = self.swash_font.as_ref();
      let shaper = self.shape_context.builder(swash_font).size(size as f32).build();
      let metrics = shaper.metrics();
      let height = metrics.ascent - metrics.descent;
      let texture = unsafe {
         let texture = gl.create_texture().unwrap();
         gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::R8 as i32,
            TEXTURE_ATLAS_SIZE as i32,
            TEXTURE_ATLAS_SIZE as i32,
            0,
            glow::RED,
            glow::UNSIGNED_BYTE,
            None,
         );
         gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MIN_FILTER,
            glow::NEAREST as i32,
         );
         gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_MAG_FILTER,
            glow::NEAREST as i32,
         );
         let swizzle_mask = [glow::ONE, glow::ONE, glow::ONE, glow::RED];
         gl.texture_swizzle_mask(glow::TEXTURE_2D, &swizzle_mask);
         texture
      };
      self.sizes.insert(
         size,
         FontSize {
            size,
            texture,
            packer: RectPacker::new(TEXTURE_ATLAS_SIZE as f32, TEXTURE_ATLAS_SIZE as f32),
            ascii: [(); 256].map(|_| None),
            unicode: HashMap::new(),
            height,
         },
      );
   }

   fn glyph_renderer(&mut self, size: u32) -> GlyphRenderer<'_, '_, '_, '_> {
      self.make_size(size);

      GlyphRenderer {
         swash_font: self.swash_font.as_ref(),
         gl: &self.gl,
         size_store: self.sizes.get_mut(&size).unwrap(),
         scale_context: &mut self.scale_context,
         shape_context: &mut self.shape_context,
      }
   }
}

impl Drop for FontFace {
   fn drop(&mut self) {
      for (_, size) in &self.sizes {
         unsafe {
            self.gl.delete_texture(size.texture);
         }
      }
   }
}

pub struct Font {
   store: Rc<RefCell<FontFace>>,
   size: u32,
}

impl Font {
   pub(crate) fn new(gl: Rc<glow::Context>, data: &[u8], default_size: f32) -> Self {
      Self {
         store: Rc::new(RefCell::new(FontFace::new(gl, data.into()).unwrap())),
         size: default_size as u32,
      }
   }

   pub(crate) fn atlas(&self) -> glow::Texture {
      let mut store = self.store.borrow_mut();
      store.make_size(self.size);
      let size_store = store.sizes.get(&self.size).unwrap();
      size_store.texture
   }

   pub(crate) fn typeset<'font, 'text>(&'font self, text: &'text str) -> Typeset<'font, 'text> {
      Typeset {
         store: self.store.borrow_mut(),
         font: self,
         text: text.chars(),
         pen_x: 0.0,
      }
   }
}

impl netcanv_renderer::Font for Font {
   fn with_size(&self, new_size: f32) -> Self {
      Self {
         store: Rc::clone(&self.store),
         size: new_size as u32,
      }
   }

   fn size(&self) -> f32 {
      self.size as f32
   }

   fn height(&self) -> f32 {
      let store = self.store.borrow();
      if let Some(size_store) = store.sizes.get(&self.size) {
         size_store.height
      } else {
         self.size()
      }
   }

   fn text_width(&self, text: &str) -> f32 {
      let typesetter = self.typeset(text);
      typesetter.fast_forward()
   }
}

pub(crate) struct GlyphRenderer<'font, 'store, 'gl, 'c> {
   swash_font: FontRef<'font>,
   size_store: &'store mut FontSize,
   gl: &'gl glow::Context,
   scale_context: &'c mut ScaleContext,
   shape_context: &'c mut ShapeContext,
}

impl<'font, 'store, 'gl, 'c> GlyphRenderer<'font, 'store, 'gl, 'c> {
   fn render_glyph(&mut self, c: char) -> anyhow::Result<Glyph> {
      let size = self.size_store.size as f32;
      let mut scaler = self.scale_context.builder(self.swash_font).size(size).hint(true).build();

      let glyph_id = self.swash_font.charmap().map(c);
      let image = Render::new(&[
         Source::ColorOutline(0),
         Source::ColorBitmap(StrikeWith::BestFit),
         Source::Outline,
      ])
      .format(Format::Alpha)
      .render(&mut scaler, glyph_id)
      .unwrap(); // TODO: handle None value later

      let rect = self
         .size_store
         .packer
         .pack(image.placement.width as _, image.placement.height as _)
         .expect("no space left on font texture atlas");
      let texture = self.size_store.texture;
      unsafe {
         self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         self.gl.tex_sub_image_2d(
            glow::TEXTURE_2D,
            0,
            rect.x() as i32,
            rect.y() as i32,
            rect.width() as i32,
            rect.height() as i32,
            glow::RED,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(&image.data),
         );
      }

      let shaper = self.shape_context.builder(self.swash_font).size(size).build();
      let glyph_metrics = self.swash_font.glyph_metrics(shaper.normalized_coords()).scale(size);

      Ok(Glyph {
         size: rect.size,
         uv_rect: rect.uv(vector(TEXTURE_ATLAS_SIZE as f32, TEXTURE_ATLAS_SIZE as f32)),
         offset: vector(image.placement.left as f32, -(image.placement.top as f32)),
         advance_x: glyph_metrics.advance_width(glyph_id),
      })
   }

   fn get_or_render_glyph(&mut self, c: char) -> anyhow::Result<&Glyph> {
      if self.size_store.get_glyph(c).is_none() {
         let glyph = self.render_glyph(c)?;
         self.size_store.insert_glyph(c, glyph);
      }
      Ok(self.size_store.get_glyph(c).unwrap())
   }
}

pub(crate) struct Typeset<'font, 'text> {
   font: &'font Font,
   store: RefMut<'font, FontFace>,
   text: Chars<'text>,
   pen_x: f32,
}

impl<'font, 'text> Typeset<'font, 'text> {
   /// Fast-forwards through the typesetting process, and yields the final pen X position.
   /// This is faster than iterating through each value of the iterator, since only the final X
   /// position is calculated, without any of the intermediate glyph positions.
   pub fn fast_forward(mut self) -> f32 {
      let mut renderer = self.store.glyph_renderer(self.font.size);
      while let Some(c) = self.text.next() {
         if let Ok(glyph) = renderer.get_or_render_glyph(c) {
            self.pen_x += glyph.advance_x;
         }
      }
      self.pen_x
   }
}

impl<'font, 'text> Iterator for Typeset<'font, 'text> {
   type Item = (Rect, Rect);

   fn next(&mut self) -> Option<Self::Item> {
      if let Some(c) = self.text.next() {
         //    Hopefully this gets hoisted out of the loop, albeit it's not that expensive in the
         // ↓ first place.
         let mut renderer = self.store.glyph_renderer(self.font.size);
         if let Ok(glyph) = renderer.get_or_render_glyph(c) {
            let pen_x = self.pen_x;
            self.pen_x += glyph.advance_x;
            Some((
               Rect::new(vector(pen_x, 0.0) + glyph.offset, glyph.size),
               glyph.uv_rect,
            ))
         } else {
            None
         }
      } else {
         None
      }
   }
}
