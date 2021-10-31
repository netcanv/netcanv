use std::rc::Rc;

use glow::HasContext;

pub struct Framebuffer {
   gl: Rc<glow::Context>,
   framebuffer: glow::Framebuffer,
   texture: glow::Texture,
   width: u32,
   height: u32,
}

impl Framebuffer {
   pub(crate) fn new(gl: Rc<glow::Context>, width: u32, height: u32) -> Self {
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
         framebuffer = gl.create_framebuffer().unwrap();
         gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
         gl.framebuffer_texture(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, Some(texture), 0);
         assert!(
            gl.check_framebuffer_status(glow::FRAMEBUFFER) == glow::FRAMEBUFFER_COMPLETE,
            "could not create framebuffer (framebuffer was incomplete)"
         );
         gl.clear_color(0.0, 0.0, 1.0, 1.0);
         gl.clear(glow::COLOR_BUFFER_BIT);
         gl.bind_framebuffer(glow::FRAMEBUFFER, None);
      }
      Framebuffer {
         gl,
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

   fn upload_rgba(&mut self, position: (u32, u32), size: (u32, u32), pixels: &[u8]) {}

   fn download_rgba(&self, dest: &mut [u8]) {}
}

impl Drop for Framebuffer {
   fn drop(&mut self) {
      unsafe {
         self.gl.delete_framebuffer(self.framebuffer);
         self.gl.delete_texture(self.texture);
      }
   }
}
