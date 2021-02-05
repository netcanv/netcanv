use std::ffi::CString;
use skulpin::app::AppBuilder;
use skulpin::LogicalSize;

mod netcanv;
mod paint_canvas;
mod ui;

use netcanv::*;

fn main() {
    let window_size = LogicalSize::new(1024, 600);
    AppBuilder::new()
        .window_title("netCanv")
        .app_name(CString::new("netCanv").unwrap())
        .inner_size(window_size)
        .use_vulkan_debug_layer(false)
        .run(NetCanv::new());
}
