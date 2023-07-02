use std::rc::Rc;

use bitflags::bitflags;
use glam::{vec2, Vec2};
use netcanv_renderer::paws::{
   vector, AlignH, AlignV, Alignment, Color, LineCap, Point, Rect, Renderer, Vector,
};
use netcanv_renderer::{BlendMode, Font as _, Framebuffer as _, RenderBackend};

use crate::common::{paws_color_to_wgpu, vector_to_vec2};
use crate::gpu::Gpu;
use crate::image::Image;
use crate::transform::{Transform, TransformState};
use crate::{Font, Framebuffer, WgpuBackend};

pub(crate) struct ClearOps {
   pub color: wgpu::Operations<wgpu::Color>,
}

impl ClearOps {
   pub fn take(&mut self) -> ClearOps {
      std::mem::take(self)
   }
}

impl Default for ClearOps {
   fn default() -> Self {
      Self {
         color: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: true,
         },
      }
   }
}

bitflags! {
   #[derive(Debug, Clone, Copy, PartialEq, Eq)]
   pub(crate) struct BlendFlags: u32 {
      const ANTIALIAS = 0x1;
      const PREMULTIPLY_ALPHA = 0x2;
   }
}

impl Default for BlendFlags {
   fn default() -> Self {
      Self::ANTIALIAS
   }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Pass {
   // NOTE: The order here must match the order of pass `flush` calls in `WgpuBackend::flush`.
   RoundedRects,
   Lines,
   Images,
   Text,
}

pub(crate) struct FlushContext<'flush> {
   pub gpu: &'flush Gpu,
   pub model_transform_bind_group: &'flush wgpu::BindGroup,
   pub scene_uniform_bind_group: &'flush wgpu::BindGroup,
   pub blend_mode: BlendMode,
}

impl WgpuBackend {
   pub(crate) fn rewind(&mut self) {
      self.last_pass = None;

      self.rounded_rects.rewind();
      self.lines.rewind();
      self.images.rewind();
      self.text.rewind();

      self.scene_uniform_cache.tick_and_evict();
   }

   fn switch_pass(&mut self, new_pass: Pass) {
      let last_pass = self.last_pass;
      self.last_pass = Some(new_pass);
      if Some(new_pass) < last_pass {
         self.flush("switch_pass");
      }
   }

   pub(crate) fn flush(&mut self, cause: &str) {
      let label = format!("Flush ({cause})");
      let mut encoder = self.gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
         label: Some(&label),
      });

      let clear_ops = self.clear_ops().take();
      let transform_state = *self.current_transform();
      let model_transform_bind_group = if let Transform::Matrix(matrix) = transform_state.transform
      {
         let (buffer, bind_group) = self.model_transform_storage.next_batch(&self.gpu);
         self.gpu.queue.write_buffer(
            buffer,
            0,
            bytemuck::bytes_of(&matrix.to_cols_array_2d().map(|[a, b, c]| [a, b, c, 0.0])),
         );
         bind_group
      } else {
         &self.identity_model_transform_bind_group
      };

      {
         let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(&label),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
               view: self.gpu.render_target(),
               resolve_target: None,
               ops: clear_ops.color,
            })],
            depth_stencil_attachment: None,
         });

         if let Some(clip_rect) = transform_state.clip {
            render_pass.set_scissor_rect(
               clip_rect.x() as u32,
               clip_rect.y() as u32,
               clip_rect.width() as u32,
               clip_rect.height() as u32,
            );
         }

         let mut context = FlushContext {
            gpu: &self.gpu,
            model_transform_bind_group,
            scene_uniform_bind_group: self
               .scene_uniform_cache
               .bind_group(&self.gpu, self.gpu.current_render_target_size),
            blend_mode: transform_state.blend_mode,
         };

         self.rounded_rects.flush(&mut context, &mut render_pass);
         self.lines.flush(&mut context, &mut render_pass);
         self.images.flush(&mut context, &self.image_storage, &mut render_pass);
         self.text.flush(&mut context, &mut self.text_renderer, &mut render_pass);
         self.last_pass = None;
      }

      self.command_buffers.push(encoder.finish());
   }

   fn clear_ops(&mut self) -> ClearOps {
      if let Some(color) = self.clear.take() {
         ClearOps {
            color: wgpu::Operations {
               load: wgpu::LoadOp::Clear(paws_color_to_wgpu(color)),
               store: true,
            },
         }
      } else {
         ClearOps::default()
      }
   }

   fn blend_flags(&self) -> BlendFlags {
      match self.current_transform().blend_mode {
         BlendMode::Replace => BlendFlags::empty(),
         BlendMode::Invert => BlendFlags::ANTIALIAS | BlendFlags::PREMULTIPLY_ALPHA,
         _ => BlendFlags::default(),
      }
   }
}

impl Renderer for WgpuBackend {
   type Font = Font;

   fn push(&mut self) {
      let transform = *self.current_transform();
      self.transform_stack.push(transform);
   }

   fn pop(&mut self) {
      let transform_state = self.current_transform();
      let next_transform_state = self.transform_stack.get(self.transform_stack.len() - 2);
      if transform_state.transform.is_matrix()
         || transform_state.clip.is_some()
         || next_transform_state.is_some_and(|ts| ts.blend_mode != transform_state.blend_mode)
      {
         self.flush("pop");
      }
      self.transform_stack.pop();
      if self.transform_stack.is_empty() {
         self.transform_stack.push(TransformState::default());
      }
   }

   fn translate(&mut self, vec: Vector) {
      let state = self.current_transform();
      self.current_transform_mut().transform = state.transform.translate(vector_to_vec2(vec));
      if self.current_transform().transform.is_matrix() {
         self.flush("translate");
      }
   }

   fn clip(&mut self, rect: Rect) {
      self.flush("clip");
      let rect = self.current_transform().transform.translate_rect(rect.sort());
      let clip = if let Some(existing_clip) = self.current_transform().clip {
         let left = existing_clip.left().max(rect.left());
         let top = existing_clip.top().max(rect.top());
         let right = existing_clip.right().min(rect.right());
         let bottom = existing_clip.bottom().min(rect.right());
         Rect::new((left, top), (right - left, bottom - top))
      } else {
         rect
      };
      self.current_transform_mut().clip = Some(clip);
   }

   fn fill(&mut self, rect: Rect, color: Color, radius: f32) {
      if color.a > 0 {
         let rect = self.current_transform().transform.translate_rect(rect);
         self.switch_pass(Pass::RoundedRects);
         self.rounded_rects.add(rect, color, radius, -1.0, self.blend_flags());
         if self.rounded_rects.needs_flush() {
            self.flush("fill");
         }
      }
   }

   fn outline(&mut self, rect: Rect, color: Color, radius: f32, thickness: f32) {
      if thickness > 0.0 && color.a > 0 {
         let rect = self.current_transform().transform.translate_rect(rect);
         self.switch_pass(Pass::RoundedRects);
         self.rounded_rects.add(rect, color, radius, thickness, self.blend_flags());
         if self.rounded_rects.needs_flush() {
            self.flush("outline");
         }
      }
   }

   fn line(&mut self, a: Point, b: Point, color: Color, cap: LineCap, thickness: f32) {
      if color.a > 0 {
         if a != b {
            let a = self.current_transform().transform.translate_vector(a);
            let b = self.current_transform().transform.translate_vector(b);
            self.switch_pass(Pass::Lines);
            self.lines.add(a, b, color, cap, thickness, self.blend_flags());
            if self.lines.needs_flush() {
               self.flush("line");
            }
         } else {
            match cap {
               LineCap::Butt => (),
               LineCap::Square => {
                  self.fill(
                     Rect::new(
                        a - vector(thickness, thickness) * 0.5,
                        vector(thickness, thickness),
                     ),
                     color,
                     thickness,
                  );
               }
               LineCap::Round => self.fill_circle(a, thickness * 0.5, color),
            }
         }
      }
   }

   fn text(
      &mut self,
      rect: Rect,
      font: &Self::Font,
      text: &str,
      color: Color,
      alignment: Alignment,
   ) -> f32 {
      let rect = self.current_transform().transform.translate_rect(rect);
      self.switch_pass(Pass::Text);

      let origin = text_origin(&rect, font, text, alignment);
      let first = self.text.glyph_index();
      let blend_flags = self.blend_flags();
      self.text_renderer.render_text(&self.gpu, font, text, origin, |pen, glyph| {
         self.text.add_glyph(pen, glyph, color, blend_flags);
      });
      let last = self.text.glyph_index();
      self.text.add_font_span(first..last, font);
      // NOTE: Text rendering doesn't flush if there isn't enough space in the buffer.
      // We operate on an unbounded buffer to make dealing with many fonts simpler.

      32.0
   }
}

fn text_origin(rect: &Rect, font: &Font, text: &str, alignment: Alignment) -> Vec2 {
   let x = match alignment.0 {
      AlignH::Left => rect.left(),
      AlignH::Center => rect.center_x() - font.text_width(text) / 2.0,
      AlignH::Right => rect.right() - font.text_width(text),
   };
   let y = match alignment.1 {
      AlignV::Top => rect.top() + font.height(),
      AlignV::Middle => rect.center_y() + font.height() / 2.0,
      AlignV::Bottom => rect.bottom(),
   };
   vec2(x.floor(), y.floor())
}

impl RenderBackend for WgpuBackend {
   type Image = Image;

   type Framebuffer = Framebuffer;

   fn create_image_from_rgba(&mut self, width: u32, height: u32, pixel_data: &[u8]) -> Self::Image {
      self.create_image_from_rgba_impl(width, height, pixel_data)
   }

   fn create_font_from_memory(&mut self, data: &[u8], default_size: f32) -> Self::Font {
      Font::new(
         Rc::clone(&self.text_renderer.caches),
         data.to_owned(),
         default_size,
      )
   }

   fn create_framebuffer(&mut self, width: u32, height: u32) -> Self::Framebuffer {
      Framebuffer::new(&self.gpu, &mut self.image_storage, width, height)
   }

   fn draw_to(&mut self, framebuffer: &Self::Framebuffer, f: impl FnOnce(&mut Self)) {
      self.flush("before draw_to");
      let target = self.gpu.current_render_target.take();
      let previous_size = self.gpu.current_render_target_size;
      self.gpu.current_render_target = Some(
         framebuffer
            .texture_view
            .take()
            .expect("draw_to may not be called reentrantly on one framebuffer"),
      );
      self.gpu.current_render_target_size = framebuffer.size();
      f(self);
      self.flush("after draw_to");
      framebuffer.texture_view.set(self.gpu.current_render_target.take());
      self.gpu.current_render_target = target;
      self.gpu.current_render_target_size = previous_size;
   }

   fn clear(&mut self, color: Color) {
      self.clear = Some(color);
   }

   fn image(&mut self, rect: Rect, image: &Self::Image) {
      if image.color.is_none() || image.color.is_some_and(|color| color.a > 0) {
         let rect = self.current_transform().transform.translate_rect(rect);
         self.switch_pass(Pass::Images);
         self.images.add(rect, image.color, image.index);
         if self.images.needs_flush() {
            self.flush("image");
         }
      }
   }

   fn framebuffer(&mut self, rect: Rect, framebuffer: &Self::Framebuffer) {
      let rect = self.current_transform().transform.translate_rect(rect);
      self.switch_pass(Pass::Images);
      self.images.add(rect, None, framebuffer.image_storage_index);
      if self.images.needs_flush() {
         self.flush("framebuffer");
      }
   }

   fn upload_framebuffer(
      &mut self,
      framebuffer: &Self::Framebuffer,
      position: (u32, u32),
      size: (u32, u32),
      pixels: &[u8],
   ) {
      framebuffer.upload(&self.gpu, position, size, pixels);
   }

   fn download_framebuffer(
      &mut self,
      framebuffer: &Self::Framebuffer,
      position: (u32, u32),
      size: (u32, u32),
      out_pixels: &mut [u8],
   ) {
      framebuffer.sync_download(&self.gpu, position, size, out_pixels);
   }

   fn scale(&mut self, scale: Vector) {
      // In case of scaling we always end up with a matrix so we need to flush whatever was about
      // to be rendered.
      self.flush("scale");
      let state = self.current_transform();
      self.current_transform_mut().transform = state.transform.scale(vector_to_vec2(scale));
   }

   fn set_blend_mode(&mut self, new_blend_mode: BlendMode) {
      if new_blend_mode != self.current_transform().blend_mode {
         self.flush("set_blend_mode");
         self.current_transform_mut().blend_mode = new_blend_mode;
      }
   }
}
