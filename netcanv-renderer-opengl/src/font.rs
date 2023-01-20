//! A <del>quite shitty</del> text renderer based on FreeType.
//!
//! Does not support advanced features such as shaping, or text wrapping.

// Not the cleanest piece of code again, but oh the things you do for a clean end user API.

use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use std::str::Chars;

use freetype::face::LoadFlag;
use freetype::Face;
use glow::{HasContext, PixelUnpackData};
use netcanv_renderer::paws::{vector, Rect, Vector};

use crate::common::{GlUtilities, RectMath};
use crate::rect_packer::RectPacker;

const TEXTURE_ATLAS_SIZE: u32 = 1024;

struct Glyph {
   uv_rect: Rect,
   size: Vector,
   offset: Vector,
   advance_x: i64,
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

struct FontFace {
   // We store a reference to the FT_Library to make sure the FT_Face doesn't
   // outlive it. Seems like freetype-rs has some lifetime safety issues.
   // https://freetype.org/freetype2/docs/reference/ft2-base_interface.html#ft_done_freetype
   gl: Rc<glow::Context>,
   _freetype: Rc<freetype::Library>,
   face: Face,
   sizes: HashMap<u32, FontSize>,
}

impl FontFace {
   fn make_size(&mut self, size: u32) {
      if self.sizes.contains_key(&size) {
         return;
      }
      let Self { gl, face, .. } = &self;
      face.set_pixel_sizes(0, size).unwrap();
      let size_metrics = face.size_metrics().unwrap();
      let height = (size_metrics.ascender - size_metrics.descender.abs()) as f32 / 64.0;
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

   fn glyph_renderer(&mut self, size: u32) -> GlyphRenderer<'_, '_, '_> {
      self.make_size(size);
      GlyphRenderer {
         face: &self.face,
         gl: &self.gl,
         size_store: self.sizes.get_mut(&size).unwrap(),
      }
   }
}

impl Drop for FontFace {
   fn drop(&mut self) {
      for size in self.sizes.values() {
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
   pub(crate) fn new(
      gl: Rc<glow::Context>,
      freetype: Rc<freetype::Library>,
      data: &[u8],
      default_size: f32,
   ) -> Self {
      let face = freetype.new_memory_face(Rc::new(data.to_owned()), 0).unwrap();
      face.set_pixel_sizes(default_size as u32, default_size as u32).unwrap();
      Self {
         store: Rc::new(RefCell::new(FontFace {
            gl,
            _freetype: freetype,
            face,
            sizes: HashMap::new(),
         })),
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
         pen_x: 0,
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

pub(crate) struct GlyphRenderer<'face, 'store, 'gl> {
   face: &'face Face,
   size_store: &'store mut FontSize,
   gl: &'gl glow::Context,
}

impl<'font, 'store, 'gl> GlyphRenderer<'font, 'store, 'gl> {
   fn render_glyph(&mut self, c: char) -> anyhow::Result<Glyph> {
      self.face.set_pixel_sizes(0, self.size_store.size)?;
      self.face.load_char(c as usize, LoadFlag::RENDER)?;
      let glyph_size = {
         let glyph = self.face.glyph();
         (glyph.bitmap().width(), glyph.bitmap().rows())
      };
      let rect = self
         .size_store
         .packer
         .pack(glyph_size.0 as _, glyph_size.1 as _)
         .expect("no space left on font texture atlas");
      let texture = self.size_store.texture;
      unsafe {
         let bitmap = self.face.glyph().bitmap();
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
            PixelUnpackData::Slice(bitmap.buffer()),
         );
      }
      let glyph = self.face.glyph();
      Ok(Glyph {
         size: rect.size,
         uv_rect: rect.uv(vector(TEXTURE_ATLAS_SIZE as f32, TEXTURE_ATLAS_SIZE as f32)),
         offset: vector(glyph.bitmap_left() as f32, -(glyph.bitmap_top() as f32)),
         advance_x: glyph.advance().x as i64,
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
   pen_x: i64,
}

impl<'font, 'text> Typeset<'font, 'text> {
   /// Fast-forwards through the typesetting process, and yields the final pen X position.
   /// This is faster than iterating through each value of the iterator, since only the final X
   /// position is calculated, without any of the intermediate glyph positions.
   pub fn fast_forward(mut self) -> f32 {
      let mut renderer = self.store.glyph_renderer(self.font.size);
      for c in self.text.by_ref() {
         if let Ok(glyph) = renderer.get_or_render_glyph(c) {
            self.pen_x += glyph.advance_x;
         }
      }
      self.pen_x as f32 / 64.0
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
               Rect::new(vector(pen_x as f32 / 64.0, 0.0) + glyph.offset, glyph.size),
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
