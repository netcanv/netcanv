use std::ffi::CString;

use skulpin::*;
use skulpin::app::{AppBuilder, AppHandler, AppUpdateArgs, AppDrawArgs, AppError};

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

    }

    fn fatal_error(&mut self, error: &AppError) {

    }

}
