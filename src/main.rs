use std::error::Error;

use skulpin::*;

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

mod app;
mod assets;
mod paint_canvas;
mod ui;
mod util;

use app::*;
use ui::input::*;

fn main() -> Result<(), Box<dyn Error>> {

    let event_loop = EventLoop::new();
    let winit_window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(1024, 600))
        .with_title("NetCanv")
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    let window = WinitWindow::new(&winit_window);
    let mut renderer = RendererBuilder::new()
        .use_vulkan_debug_layer(false)
        .build(&window)?;

    let mut app: Box<dyn AppState> = Box::new(paint::State::new()) as _;
    let mut input = Input::new();

    event_loop.run(move |event, _, control_flow| {
        let window = WinitWindow::new(&winit_window);
        *control_flow = ControlFlow::Wait;

        match event {

            Event::WindowEvent {
                event,
                ..
            } => {
                if let WindowEvent::CloseRequested = event {
                    *control_flow = ControlFlow::Exit;
                } else {
                    input.process_event(&event);
                }
            },

            Event::MainEventsCleared => {
                renderer.draw(&window, |canvas, csh| {
                    let next = app.process(StateArgs {
                        canvas,
                        coordinate_system_helper: &csh,
                        input: &mut input,
                    });
                    if let Some(state) = next {
                        app = state;
                    }
                }).unwrap();
                input.finish_frame();
            },

            _ => (),

        }
    });
}
