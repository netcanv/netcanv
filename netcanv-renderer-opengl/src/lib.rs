mod common;
mod font;
mod framebuffer;
mod image;
mod rendering;

use glow::{Context, HasContext};
use glutin::{Api, ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};
use netcanv_renderer::paws::{Renderer, Ui};
use netcanv_renderer::RenderBackend;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use crate::{font::Font, framebuffer::Framebuffer, image::Image};

pub struct OpenGlBackend {
   context: WindowedContext<PossiblyCurrent>,
   pub(crate) gl: glow::Context,
}

impl OpenGlBackend {
   /// Creates a new OpenGL renderer.
   pub fn new(window_builder: WindowBuilder, event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
      let context = ContextBuilder::new()
         .with_gl(GlRequest::Latest)
         .with_gl_profile(GlProfile::Core)
         .with_vsync(true)
         // .with_multisampling(8)
         // .with_double_buffer(Some(true))
         .build_windowed(window_builder, event_loop)?;
      let context = unsafe { context.make_current().unwrap() };
      let gl = unsafe {
         glow::Context::from_loader_function(|name| context.get_proc_address(name) as *const _)
      };
      Ok(Self { context, gl })
   }

   /// Returns the window.
   pub fn window(&self) -> &Window {
      self.context.window()
   }
}

pub trait UiRenderFrame {
   /// Renders a single frame onto the window.
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<OpenGlBackend> {
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      unsafe {
         self.gl.clear_color(0.0, 0.0, 1.0, 1.0);
         self.gl.clear(glow::COLOR_BUFFER_BIT);
      }
      self.context.swap_buffers()?;
      Ok(())
   }
}
