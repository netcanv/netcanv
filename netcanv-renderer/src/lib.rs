use paws::Renderer;
use winit::window::Window;

pub trait Font {
   fn from_memory(memory: &[u8], default_size: f32) -> Self;

   fn height(&self) -> f32;
   fn text_width(&self, text: &str) -> f32;
}

pub trait Image {
   fn from_rgba(width: usize, height: usize, pixel_data: &[u8]) -> Self;
}

pub trait RenderBackend: Renderer {
   fn render(&mut self, window: &Window, callback: impl FnOnce()) -> anyhow::Result<()>;
}
