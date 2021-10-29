mod common;
mod font;
mod framebuffer;
mod image;
mod rendering;

use std::rc::Rc;

use glow::HasContext;
use glutin::{Api, ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, WindowedContext};
use netcanv_renderer::paws::{point, Ui};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use crate::{font::Font, framebuffer::Framebuffer, image::Image};
use rendering::RenderState;

pub struct OpenGlBackend {
   context: WindowedContext<PossiblyCurrent>,
   pub(crate) gl: Rc<glow::Context>,
   state: RenderState,
}

impl OpenGlBackend {
   /// Creates a new OpenGL renderer.
   pub fn new(window_builder: WindowBuilder, event_loop: &EventLoop<()>) -> anyhow::Result<Self> {
      let context = ContextBuilder::new()
         .with_gl(GlRequest::Specific(Api::OpenGlEs, (3, 0)))
         .with_gl_profile(GlProfile::Core)
         .with_vsync(true)
         .with_multisampling(8)
         .build_windowed(window_builder, event_loop)?;
      let context = unsafe { context.make_current().unwrap() };
      let gl = unsafe {
         glow::Context::from_loader_function(|name| context.get_proc_address(name) as *const _)
      };
      let gl = Rc::new(gl);
      Ok(Self {
         context,
         state: RenderState::new(Rc::clone(&gl)),
         gl,
      })
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
      callback(self);
      self.state.draw(
         &[
            point(0.0, 0.5).into(),
            point(-0.5, -0.5).into(),
            point(0.5, -0.5).into(),
         ],
         &[0, 1, 2],
      );
      self.context.swap_buffers()?;
      Ok(())
   }
}
