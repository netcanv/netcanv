use std::cell::RefCell;
use std::rc::Rc;

use anyhow::anyhow;

use super::caches::Caches;

#[derive(Clone)]
pub struct Font {
   pub(super) data: Rc<FontData>,
   pub(super) caches: Rc<RefCell<Caches>>,
   pub(super) size: f32,
}

impl Font {
   pub(crate) fn new(caches: Rc<RefCell<Caches>>, data: Vec<u8>, size: f32) -> Self {
      Self {
         // TODO: Could probably use better error handling.
         data: Rc::new(FontData::new(data).expect("failed to load font")),
         caches,
         size,
      }
   }

   const SIZE_GRANULARITY: f32 = 0.5;

   pub(crate) fn size_key(&self) -> u32 {
      (self.size / Self::SIZE_GRANULARITY) as u32
   }

   pub(crate) fn normalized_size(&self) -> f32 {
      self.size_key() as f32 * Self::SIZE_GRANULARITY
   }

   pub(crate) fn key(&self) -> (swash::CacheKey, u32) {
      (self.data.key, self.size_key())
   }
}

impl netcanv_renderer::Font for Font {
   fn with_size(&self, new_size: f32) -> Self {
      Font {
         data: Rc::clone(&self.data),
         caches: Rc::clone(&self.caches),
         size: new_size,
      }
   }

   fn size(&self) -> f32 {
      self.size
   }

   fn height(&self) -> f32 {
      let metrics = self.data.as_font_ref().metrics(&[]).scale(self.size);
      metrics.ascent - metrics.descent
   }

   fn text_width(&self, text: &str) -> f32 {
      let mut caches = self.caches.borrow_mut();
      let mut shaper =
         caches.shape_context.builder(self.data.as_font_ref()).size(self.size).build();

      let mut pen_x = 0.0;
      shaper.add_str(text);
      shaper.shape_with(|glyph_cluster| {
         pen_x += glyph_cluster.advance();
      });

      pen_x
   }
}

impl std::fmt::Debug for Font {
   fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
      f.debug_struct("Font").field("size", &self.size).finish_non_exhaustive()
   }
}

pub(crate) struct FontData {
   data: Vec<u8>,
   offset: u32,
   pub key: swash::CacheKey,
}

impl FontData {
   pub fn new(data: Vec<u8>) -> anyhow::Result<Self> {
      let swash::FontRef { offset, key, .. } =
         swash::FontRef::from_index(&data, 0).ok_or_else(|| anyhow!("Failed to load font"))?;
      Ok(Self { data, offset, key })
   }

   pub(super) fn as_font_ref(&self) -> swash::FontRef<'_> {
      swash::FontRef {
         data: &self.data,
         offset: self.offset,
         key: self.key,
      }
   }
}
