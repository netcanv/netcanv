use std::{borrow::Borrow, error::Error};

use skulpin::*;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_os = "linux")]
use winit::platform::unix::WindowBuilderExtUnix;
use winit::window::WindowBuilder;

mod app;
mod assets;
mod net;
mod paint_canvas;
mod ui;
mod util;
mod viewport;

use app::*;
use assets::*;
use ui::input::*;

#[cfg(target_family = "unix")]
use winit::platform::unix::*;

#[cfg(target_family = "unix")]
fn winit_argb_from_skia_color(color: skia_safe::Color) -> ARGBColor {
    ARGBColor {
        a: color.a(),
        r: color.r(),
        g: color.g(),
        b: color.b(),
    }
}

#[cfg(target_family = "unix")]
impl Theme for assets::ColorScheme {
    fn element_color(&self, element: Element, window_active: bool) -> ARGBColor {
        match element {
            Element::Bar => winit_argb_from_skia_color(self.text_field.fill),
            Element::Separator => winit_argb_from_skia_color(self.text_field.text_hint),
            Element::Text => winit_argb_from_skia_color(self.text_field.text),
        }
    }

    fn button_color(&self, button: Button, state: ButtonState, foreground: bool, _window_active: bool) -> ARGBColor {
        let color: ARGBColor;

        match button {
            Button::Close => color = winit_argb_from_skia_color(self.error),
            Button::Maximize => color = winit_argb_from_skia_color(self.text),
            Button::Minimize => color = winit_argb_from_skia_color(self.text),
        }

        if foreground {
            if state == ButtonState::Hovered {
                return winit_argb_from_skia_color(self.panel);
            } else {
                return winit_argb_from_skia_color(self.text_field.text);
            }
        }

        match state {
            ButtonState::Disabled => winit_argb_from_skia_color(self.text_field.text_hint),
            ButtonState::Hovered => color,
            ButtonState::Idle => winit_argb_from_skia_color(self.text_field.fill),
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let event_loop = EventLoop::new();
    let winit_window = {
        let mut b = WindowBuilder::new()
            .with_inner_size(LogicalSize::new(1024, 600))
            .with_title("NetCanv")
            .with_resizable(true);
        #[cfg(target_os = "linux")]
        {
            b = b.with_app_id("netcanv".into())
        }
        b
    }
    .build(&event_loop)?;

    #[cfg(target_family = "unix")]
    winit_window.set_wayland_theme(ColorScheme::light());

    let window = WinitWindow::new(&winit_window);
    let mut renderer = RendererBuilder::new().use_vulkan_debug_layer(false).build(&window)?;

    let assets = Assets::new(ColorScheme::light());
    let mut app: Option<Box<dyn AppState>> = Some(Box::new(lobby::State::new(assets, None)) as _);
    let mut input = Input::new();

    event_loop.run(move |event, _, control_flow| {
        let window = WinitWindow::new(&winit_window);
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } => {
                if let WindowEvent::CloseRequested = event {
                    *control_flow = ControlFlow::Exit;
                } else {
                    input.process_event(&event);
                }
            }

            Event::MainEventsCleared => {
                match renderer.draw(&window, |canvas, csh| {
                    // unwrap always succeeds here as app is never None
                    // i don't really like this method chaining tho
                    app.as_mut().unwrap().process(StateArgs {
                        canvas,
                        coordinate_system_helper: &csh,
                        input: &mut input,
                    });
                    app = Some(app.take().unwrap().next_state());
                }) {
                    Err(error) => eprintln!("render error: {}", error),
                    _ => (),
                };
                input.finish_frame();
            }

            _ => (),
        }
    });
}
