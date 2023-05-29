pub mod cli;
mod common;
mod font;
mod framebuffer;
mod image;
mod rect_packer;
mod rendering;
mod shape_buffer;

use std::fmt::Write;
use std::rc::Rc;

use cli::RendererCli;
use glutin::dpi::PhysicalSize;
use glutin::{
   ContextBuilder, ContextWrapper, GlProfile, GlRequest, NotCurrent, PossiblyCurrent,
   WindowedContext,
};
use netcanv_renderer::paws::Ui;
pub use winit;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use crate::font::Font;
pub use crate::framebuffer::Framebuffer;
pub use crate::image::Image;
use rendering::RenderState;

pub struct OpenGlBackend {
   context: WindowedContext<PossiblyCurrent>,
   context_size: PhysicalSize<u32>,
   pub(crate) gl: Rc<glow::Context>,
   state: RenderState,
}

impl OpenGlBackend {
   fn build_context(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
   ) -> anyhow::Result<ContextWrapper<NotCurrent, Window>> {
      struct Configuration {
         msaa: u16,
         error: String,
      }
      let mut attempted_configurations = Vec::new();
      let mut successful_configuration = None;

      // Multiply MSAA value by 2, because it's divided by 2 before construction.
      // This gives us a maximum MSAA value of 8, and minimum of 0.
      let mut msaa: u16 = 8 * 2;
      while msaa > 0 {
         let mut context = ContextBuilder::new()
            .with_gl(GlRequest::Latest)
            .with_gl_profile(GlProfile::Core)
            .with_vsync(true)
            .with_multisampling(msaa);
         if msaa > 0 {
            msaa /= 2;
            context = context.with_multisampling(msaa);
         }

         match context.build_windowed(window_builder.clone(), event_loop) {
            Ok(ok) => {
               successful_configuration = Some(ok);
               break;
            }
            Err(error) => {
               attempted_configurations.push(Configuration {
                  msaa,
                  error: error.to_string(),
               });
            }
         }
      }

      if let Some(configuration) = successful_configuration {
         Ok(configuration)
      } else {
         let mut error_message = String::from(
            "Failed to create OpenGL context.\nTried the following configurations, none of which seem to be supported:\n",
         );
         for Configuration { msaa, error } in &attempted_configurations {
            let _ = writeln!(
               error_message,
               " - Multisampling: {:?}; failed with: '{}'",
               msaa, error
            );
         }
         error_message.push_str("Try updating your graphics drivers. If that doesn't help, NetCanv is too new to run on your hardware!");
         anyhow::bail!(error_message)
      }
   }

   /// Creates a new OpenGL renderer.
   pub async fn new(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
      _: &RendererCli,
   ) -> anyhow::Result<Self> {
      let context = Self::build_context(window_builder, event_loop)?;
      let context = unsafe { context.make_current().unwrap() };
      let gl = unsafe {
         glow::Context::from_loader_function(|name| context.get_proc_address(name) as *const _)
      };
      let gl = Rc::new(gl);
      Ok(Self {
         context_size: context.window().inner_size(),
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
      let window_size = self.window().inner_size();
      if self.context_size != window_size {
         self.context.resize(window_size);
      }
      self.state.viewport(window_size.width, window_size.height);
      callback(self);
      self.context.swap_buffers()?;
      Ok(())
   }
}
