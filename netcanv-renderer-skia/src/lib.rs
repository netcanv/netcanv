mod conversions;
mod rendering;

use netcanv_renderer::RenderBackend;
use paws::{Color, Point, Ui};
use skulpin::skia_safe::{
   AlphaType, Canvas, ColorType, ISize, ImageInfo, SamplingOptions, Surface,
};
use skulpin::{rafx::api::RafxExtents2D, Renderer, RendererBuilder};
use winit::dpi::PhysicalSize;
use winit::window::Window;

use conversions::*;
pub use rendering::*;

struct SurfaceInner {
   inner: Option<Surface>,
}

impl SurfaceInner {
   /// Reinitializes the window surface.
   fn initialize(&mut self, canvas: &mut Canvas, window: &Window) {
      let PhysicalSize { width, height } = window.inner_size();
      let surface = canvas
         .new_surface(
            &ImageInfo::new(
               ISize::new(width as i32, height as i32),
               ColorType::RGBA8888,
               AlphaType::Opaque,
               None,
            ),
            None,
         )
         .expect("netcanv-renderer-skia: could not create window-sized surface");
      self.inner = Some(surface);
   }
}

pub struct SkiaBackend {
   renderer: Option<Box<Renderer>>,
   // We can't simply store a reference to the canvas we're given by skulpin, because its lifetime
   // doesn't match the lifetime of the struct itself. Thus, we have an extra layer of indirection
   // in form of this surface.
   // We also need to dance around the fact that we can only create hardware-accelerated surfaces
   // only if we already have a canvas, so we initially keep this uninitialized.
   surface: SurfaceInner,
}

impl SkiaBackend {
   /// Sets the backend up for rendering.
   pub fn new(window: &Window) -> anyhow::Result<Self> {
      let extents = get_window_extents(window);
      let renderer = RendererBuilder::new().build(window, extents.clone())?;
      Ok(Self {
         renderer: Some(Box::new(renderer)),
         surface: SurfaceInner { inner: None },
      })
   }

   pub(crate) fn canvas(&mut self) -> &mut Canvas {
      self.surface.inner.as_mut().expect("use of uninitialized surface").canvas()
   }
}

impl RenderBackend for SkiaBackend {
   type Image = Image;
   type Framebuffer = Framebuffer;

   fn create_framebuffer(&self, width: usize, height: usize) -> Self::Framebuffer {
      Framebuffer {}
   }

   fn clear(&mut self, color: Color) {
      self.canvas().clear(to_color(color));
   }

   fn image(&mut self, point: Point, image: &Self::Image) {
      self.canvas().draw_image(&image.image, to_point(point), None);
   }
}

pub trait UiRenderFrame {
   fn render_frame(
      &mut self,
      window: &Window,
      callback: impl FnOnce(&mut Self),
   ) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<SkiaBackend> {
   fn render_frame(
      &mut self,
      window: &Window,
      callback: impl FnOnce(&mut Self),
   ) -> anyhow::Result<()> {
      let extents = get_window_extents(window);
      let mut renderer = self.renderer.take().expect("render() calls must not be nested");
      renderer.draw(
         extents,
         window.scale_factor(),
         |canvas, _coordinate_system_helper| {
            // Initialize the surface if this is the first frame.
            if self.surface.inner.is_none() {
               self.surface.initialize(canvas, window);
            }
            // Also reinitialize the surface if the window has been resized.
            let surface_inner = self.surface.inner.as_ref().unwrap();
            let PhysicalSize { width, height } = window.inner_size();
            if surface_inner.width() != width as i32 || surface_inner.height() != height as i32 {
               self.surface.initialize(canvas, window);
            }
            // Execute user drawing code.
            callback(self);
            // Draw the surface to the screen.
            self.surface.inner.as_mut().unwrap().draw(
               canvas,
               (0, 0),
               SamplingOptions::default(),
               None,
            );
         },
      )?;
      self.renderer = Some(renderer);
      Ok(())
   }
}

/// Returns the rafx extents for the window.
fn get_window_extents(window: &Window) -> RafxExtents2D {
   RafxExtents2D {
      width: window.inner_size().width,
      height: window.inner_size().height,
   }
}
