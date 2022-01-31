use std::cell::RefCell;
use std::rc::Rc;

use glow::{HasContext, PixelPackData, PixelUnpackData};
use netcanv_renderer::ScalingFilter;

use crate::common::flip_vertically;
use crate::rendering::GlState;

pub struct Framebuffer {
   gl: Rc<glow::Context>,
   framebuffer: glow::Framebuffer,
   texture: glow::Texture,
   width: u32,
   height: u32,
   gl_state: Rc<RefCell<GlState>>,
}

impl Framebuffer {
   pub(crate) fn new(
      gl: Rc<glow::Context>,
      gl_state: Rc<RefCell<GlState>>,
      width: u32,
      height: u32,
   ) -> Self {
      let texture;
      let framebuffer;
      unsafe {
         texture = gl.create_texture().unwrap();
         gl.bind_texture(glow::TEXTURE_2D, Some(texture));
         gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            width as i32,
            height as i32,
            0,
            glow::RGBA,
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
         gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_S,
            glow::CLAMP_TO_EDGE as i32,
         );
         gl.tex_parameter_i32(
            glow::TEXTURE_2D,
            glow::TEXTURE_WRAP_T,
            glow::CLAMP_TO_EDGE as i32,
         );
         framebuffer = gl.create_framebuffer().unwrap();
         gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
         gl.framebuffer_texture(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, Some(texture), 0);
         assert!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE,
            "could not create framebuffer (framebuffer was incomplete)"
         );
         gl.clear_color(0.0, 0.0, 0.0, 0.0);
         gl.clear(glow::COLOR_BUFFER_BIT);
         gl.bind_framebuffer(glow::FRAMEBUFFER, None);
      }
      Framebuffer {
         gl,
         gl_state,
         texture,
         framebuffer,
         width,
         height,
      }
   }

   pub(crate) fn framebuffer(&self) -> glow::Framebuffer {
      self.framebuffer
   }

   pub(crate) fn texture(&self) -> glow::Texture {
      self.texture
   }
}

impl netcanv_renderer::Framebuffer for Framebuffer {
   fn size(&self) -> (u32, u32) {
      (self.width, self.height)
   }

   fn upload_rgba(&mut self, (x, mut y): (u32, u32), (width, height): (u32, u32), pixels: &[u8]) {
      let mut flipped = pixels.to_owned();
      flip_vertically(width as usize, height as usize, 4, &mut flipped);
      y = self.height - y - height;
      unsafe {
         self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
         self.gl.tex_sub_image_2d(
            glow::TEXTURE_2D,
            0,
            x as i32,
            y as i32,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelUnpackData::Slice(&flipped),
         );
      }
   }

   fn download_rgba(&self, (x, y): (u32, u32), (width, height): (u32, u32), dest: &mut [u8]) {
      assert!(
         dest.len() == width as usize * height as usize * 4,
         "destination buffer's size must match the framebuffer texture's size"
      );
      // Read the pixels.
      unsafe {
         let mut gl_state = self.gl_state.borrow_mut();
         let previous_framebuffer = gl_state.framebuffer(&self.gl, Some(self.framebuffer));
         self.gl.read_pixels(
            x as i32,
            (self.height - y - height) as i32,
            width as i32,
            height as i32,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            PixelPackData::Slice(dest),
         );
         gl_state.framebuffer(&self.gl, previous_framebuffer);
      }
      // Fleeeeeeeeeeep them 'round.
      flip_vertically(width as usize, height as usize, 4, dest);
   }

   fn set_scaling_filter(&mut self, filter: ScalingFilter) {
      unsafe {
         self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
         let filter = match filter {
            ScalingFilter::Nearest => glow::NEAREST,
            ScalingFilter::Linear => glow::LINEAR,
         } as i32;
         self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);
         self.gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
      }
   }
}

impl Drop for Framebuffer {
   fn drop(&mut self) {
      unsafe {
         self.gl.delete_framebuffer(self.framebuffer);
         self.gl.delete_texture(self.texture);
      }
   }
}
