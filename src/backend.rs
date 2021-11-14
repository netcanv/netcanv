//! Backend selection.

// Change _only this line_ to select a different backend. This should be replaced with features
// soon enough.

#[cfg(feature = "renderer-skia")]
use netcanv_renderer_skia::{self as the_backend, SkiaBackend as TheBackend};

#[cfg(feature = "renderer-opengl")]
use netcanv_renderer_opengl::{self as the_backend, OpenGlBackend as TheBackend};

pub use the_backend::winit;

pub type Backend = TheBackend;
pub type Image = the_backend::Image;
pub type Font = the_backend::Font;
pub type Framebuffer = the_backend::Framebuffer;

// Check if the provided types implement renderer traits.

trait Requirements {
   type Backend: netcanv_renderer::RenderBackend;
   type Font: netcanv_renderer::Font;
   type Image: netcanv_renderer::Image;
   type Framebuffer: netcanv_renderer::Framebuffer;
}

enum Assertions {}

impl Requirements for Assertions {
   type Backend = Backend;
   type Font = Font;
   type Image = Image;
   type Framebuffer = Framebuffer;
}
