use std::ffi::CString;

use skulpin::*;
use skulpin::app::{AppBuilder, AppHandler, AppUpdateArgs, AppDrawArgs, AppError};
use skulpin::skia_safe::*;

pub struct NetCanv {
}

impl NetCanv {

    pub fn new() -> Self {
        NetCanv {}
    }

    pub fn build() -> ! {
        let window_size = LogicalSize::new(1024, 600);
        AppBuilder::new()
            .window_title("netCanv")
            .app_name(CString::new("netCanv").unwrap())
            .inner_size(window_size)
            .use_vulkan_debug_layer(false)
            .run(Self::new());
    }

}

impl AppHandler for NetCanv {

    fn update(&mut self, args: AppUpdateArgs) {
    }

    fn draw(&mut self, args: AppDrawArgs) {
        let canvas = args.canvas;
        canvas.clear(Color::WHITE);

        let black = Color4f::new(0.0, 0.0, 0.0, 1.0);
        let mut black_fill = Paint::new(black, None);
        black_fill.set_anti_alias(true);
        black_fill.set_style(paint::Style::Fill);

        canvas.draw_rect(Rect {
            left: 32.0,
            top: 32.0,
            right: 64.0,
            bottom: 64.0,
        }, &black_fill);
    }

    fn fatal_error(&mut self, error: &AppError) {

    }

}
