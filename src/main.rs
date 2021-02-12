use std::ffi::CString;
use skulpin::app::AppBuilder;

use winit::dpi::LogicalSize;
use winit::event::{Event, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;

mod netcanv;
mod paint_canvas;
mod ui;
mod util;

use netcanv::*;

fn main() {
//     let window_size = LogicalSize::new(1024, 600);
//     AppBuilder::new()
//         .window_title("netCanv")
//         .app_name(CString::new("netCanv").unwrap())
//         .inner_size(window_size)
//         .use_vulkan_debug_layer(false)
//         .run(NetCanv::new());

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(LogicalSize::new(1024, 600))
        .with_title("NetCanv")
        .with_resizable(true)
        .build(&event_loop)
        .unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {

            Event::WindowEvent {
                event,
                ..
            } => {
                if let WindowEvent::CloseRequested = event {
                    *control_flow = ControlFlow::Exit;
                }
            },

            Event::MainEventsCleared => {
                window.request_redraw();
            },

            Event::RedrawRequested(_) => {

            },

            _ => (),

        }
    });
}
