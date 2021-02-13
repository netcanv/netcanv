use std::error::Error;

use skulpin::*;
use skulpin::skia_safe::*;

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

mod netcanv;
mod paint_canvas;
mod ui;
mod util;

use netcanv::*;
use ui::input::*;

fn main() -> Result<(), Box<dyn Error>> {
//     let window_size = LogicalSize::new(1024, 600);
//     AppBuilder::new()
//         .window_title("netCanv")
//         .app_name(CString::new("netCanv").unwrap())
//         .inner_size(window_size)
//         .use_vulkan_debug_layer(false)
//         .run(NetCanv::new());

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

    let mut app = NetCanv::new();
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
                renderer.draw(&window, |canvas, coordinate_system_helper| {
                    app.process(canvas, &input).unwrap();
                }).unwrap();
                input.finish_frame();
            },

            _ => (),

        }
    });
}
