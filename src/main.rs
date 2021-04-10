use std::error::Error;

use skulpin::rafx::api::RafxExtents2D;
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

fn main() -> Result<(), Box<dyn Error>> {
    let window_size = LogicalSize::new(1024, 600);

    let event_loop = EventLoop::new();
    let window = {
        let mut b = WindowBuilder::new()
            .with_inner_size(window_size)
            .with_title("NetCanv")
            .with_resizable(true);
        #[cfg(target_os = "linux")]
        {
            b = b.with_app_id("netcanv".into())
        }
        b
    }
    .build(&event_loop)?;

    let mut renderer = RendererBuilder::new()
        .coordinate_system(CoordinateSystem::Logical)
        .build(&window, RafxExtents2D {
            width: window_size.width,
            height: window_size.height,
        })?;

    let assets = Assets::new(ColorScheme::light());
    let mut app: Option<Box<dyn AppState>> = Some(Box::new(lobby::State::new(assets, None)) as _);
    let mut input = Input::new();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;

        match event {
            Event::WindowEvent { event, .. } =>
                if let WindowEvent::CloseRequested = event {
                    *control_flow = ControlFlow::Exit;
                } else {
                    input.process_event(&event);
                },

            Event::MainEventsCleared => {
                let window_size = window.inner_size().to_logical(window.scale_factor());
                let window_extents = RafxExtents2D {
                    width: window_size.width,
                    height: window_size.height,
                };
                match renderer.draw(window_extents, window.scale_factor(), |canvas, csh| {
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

            _ => (),
        }
    });
}
