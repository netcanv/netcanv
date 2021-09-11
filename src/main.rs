#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;

use config::UserConfig;
use skulpin::*;
use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
#[cfg(target_family = "unix")]
use winit::platform::unix::*;
use winit::window::WindowBuilder;

mod app;
mod assets;
mod config;
mod net;
mod paint_canvas;
mod ui;
mod util;
mod viewport;

use app::*;
use assets::*;
use ui::input::*;

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

    let mut config = UserConfig::load_or_create()?;
    let color_scheme = match config.ui.color_scheme {
        config::ColorScheme::Light => ColorScheme::light(),
        config::ColorScheme::Dark => ColorScheme::dark(),
    };

    #[cfg(target_family = "unix")]
    winit_window.set_wayland_theme(color_scheme.clone());

    let window = WinitWindow::new(&winit_window);
    let mut renderer = RendererBuilder::new().use_vulkan_debug_layer(false).build(&window)?;

    let assets = Assets::new(color_scheme);
    let mut app: Option<Box<dyn AppState>> = Some(Box::new(lobby::State::new(assets, config, None)) as _);
    let mut input = Input::new();

    event_loop.run(move |event, _, control_flow| {
        let window = WinitWindow::new(&winit_window);
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } =>
                if let WindowEvent::CloseRequested = event {
                    *control_flow = ControlFlow::Exit;
                } else {
                    input.process_event(&event);
                },

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
            },

            Event::LoopDestroyed => {
                // Fix for SIGSEGV inside of skia-[un]safe due to a Surface not being dropped properly.
                // Not sure what that's all about, but this little snippet fixes the bug so eh, why not.
                drop(app.take().unwrap());
            },

            _ => (),
        }
    });
}
