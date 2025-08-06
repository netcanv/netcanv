pub mod cli;
mod common;
mod font;
mod framebuffer;
mod image;
mod rect_packer;
mod rendering;
mod shape_buffer;

use std::ffi::CString;
use std::num::NonZeroU32;
use std::rc::Rc;

use cli::RendererCli;
use glutin::config::{Config, ConfigTemplateBuilder, GlConfig};
use glutin::context::{
   ContextApi, ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext,
};
use glutin::display::{GetGlDisplay, GlDisplay};
use glutin::prelude::*;
use glutin::surface::{Surface, WindowSurface};
use glutin_winit::{DisplayBuilder, GlWindow};
use netcanv_renderer::paws::Ui;
use raw_window_handle::HasRawWindowHandle;
pub use winit;
use winit::dpi::PhysicalSize;
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowBuilder};

pub use crate::font::Font;
pub use crate::framebuffer::Framebuffer;
pub use crate::image::Image;
use rendering::RenderState;

pub struct OpenGlBackend {
   context: PossiblyCurrentContext,
   surface: Surface<WindowSurface>,
   window: Window,
   window_size: PhysicalSize<u32>,
   pub(crate) gl: Rc<glow::Context>,
   state: RenderState,
}

impl OpenGlBackend {
   fn build_context(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
   ) -> anyhow::Result<(NotCurrentContext, Config, Window)> {
      let template = ConfigTemplateBuilder::new().with_multisampling(8);

      // Passing window_builder is required by Windows.
      // On Android, it should be passed later, but because we don't care about Android, we can take
      // a shortcut.
      let display_builder = DisplayBuilder::new().with_window_builder(Some(window_builder));

      let (window, gl_config) = display_builder
         .build(event_loop, template, |configs| {
            configs
               .reduce(|accum, config| {
                  // Find the config with the maximum number of samples
                  if config.num_samples() > accum.num_samples() {
                     config
                  } else {
                     accum
                  }
               })
               .unwrap()
         })
         .map_err(|_| anyhow::anyhow!("Failed to create OpenGL window"))?;

      let raw_window_handle = window.as_ref().map(|window| window.raw_window_handle());

      let gl_display = gl_config.display();

      // Default modern OpenGL core
      let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);

      // In case if modern OpenGL isn't present, fallback to OpenGL ES
      let fallback_context_attributes = ContextAttributesBuilder::new()
         .with_context_api(ContextApi::Gles(None))
         .build(raw_window_handle);

      let not_current_gl_context = unsafe {
         gl_display
            .create_context(&gl_config, &context_attributes)
            .or_else(|_| gl_display.create_context(&gl_config, &fallback_context_attributes))
      }?;

      let window = window.ok_or(anyhow::anyhow!("Failed to create OpenGL window"))?;

      Ok((not_current_gl_context, gl_config, window))
   }

   /// Creates a new OpenGL renderer.
   pub async fn new(
      window_builder: WindowBuilder,
      event_loop: &EventLoop<()>,
      _: &RendererCli,
   ) -> anyhow::Result<Self> {
      let (context, gl_config, window) = Self::build_context(window_builder, event_loop)?;
      let window_size = window.inner_size();

      let attributes = window.build_surface_attributes(<_>::default());
      let surface = unsafe { gl_config.display().create_window_surface(&gl_config, &attributes) }?;

      let context = context.make_current(&surface)?;

      let gl = unsafe {
         glow::Context::from_loader_function(|name| {
            let name = CString::new(name).unwrap();
            gl_config.display().get_proc_address(&name) as *const _
         })
      };
      let gl = Rc::new(gl);
      Ok(Self {
         context,
         surface,
         window,
         window_size,
         state: RenderState::new(Rc::clone(&gl)),
         gl,
      })
   }

   /// Returns the window.
   pub fn window(&self) -> &Window {
      &self.window
   }

   /// Resize the window.
   pub fn resize(&mut self, size: PhysicalSize<u32>) {
      if size.width > 0 && size.height > 0 {
         self.window_size = size;
         self.surface.resize(
            &self.context,
            NonZeroU32::new(size.width).unwrap(),
            NonZeroU32::new(size.height).unwrap(),
         );
      }
   }

   /// Swap buffers.
   pub fn swap_buffers(&self) -> glutin::error::Result<()> {
      self.surface.swap_buffers(&self.context)
   }
}

pub trait UiRenderFrame {
   /// Renders a single frame onto the window.
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()>;
}

impl UiRenderFrame for Ui<OpenGlBackend> {
   fn render_frame(&mut self, callback: impl FnOnce(&mut Self)) -> anyhow::Result<()> {
      let window_size = self.window().inner_size();
      if self.window_size != window_size {
         self.resize(window_size);
      }
      self.state.viewport(window_size.width, window_size.height);
      callback(self);
      self.swap_buffers()?;
      Ok(())
   }
}
