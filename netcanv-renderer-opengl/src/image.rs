use std::{
   cell::{Cell, RefCell},
   rc::Rc,
};

use glow::{HasContext, NativeTexture};
use netcanv_renderer::paws::Color;

pub(crate) enum ImageState {
   Queued(Vec<u8>),
   Uploading,
   Ready(Rc<glow::Context>, NativeTexture),
}

impl Drop for ImageState {
   fn drop(&mut self) {
      match self {
         Self::Ready(gl, texture) => unsafe {
            gl.delete_texture(*texture);
         },
         _ => (),
      }
   }
}

pub struct Image {
   width: u32,
   height: u32,
   pub(crate) color: Option<Color>,
   pub(crate) state: Rc<Cell<ImageState>>,
}

impl Image {
   pub(crate) fn upload(&self, gl: &Rc<glow::Context>) -> NativeTexture {
      let state = self.state.replace(ImageState::Uploading);
      match &state {
         ImageState::Queued(pixels) => unsafe {
            let texture = gl.create_texture().unwrap();
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_image_2d(
               glow::TEXTURE_2D,
               0,
               glow::RGBA as i32,
               self.width as i32,
               self.height as i32,
               0,
               glow::RGBA,
               glow::UNSIGNED_BYTE,
               Some(pixels),
            );
            gl.generate_mipmap(glow::TEXTURE_2D);
            self.state.set(ImageState::Ready(Rc::clone(gl), texture));
            texture
         },
         ImageState::Uploading => unreachable!(),
         ImageState::Ready(_, texture) => {
            let texture = *texture;
            self.state.set(state);
            return texture;
         }
      }
   }
}

impl netcanv_renderer::Image for Image {
   fn from_rgba(width: u32, height: u32, pixel_data: &[u8]) -> Self {
      Self {
         width,
         height,
         color: None,
         state: Rc::new(Cell::new(ImageState::Queued(pixel_data.into()))),
      }
   }

   fn colorized(&self, color: Color) -> Self {
      Self {
         width: self.width,
         height: self.height,
         color: Some(color),
         state: Rc::clone(&self.state),
      }
   }

   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }
}
