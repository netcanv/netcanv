use skulpin::skia_safe::*;
use skulpin::CoordinateSystemHelper;

use crate::config::UserConfig;
use crate::ui::*;

pub struct StateArgs<'a, 'b, 'c, 'd> {
    pub canvas: &'a mut Canvas,
    pub coordinate_system_helper: &'b CoordinateSystemHelper,
    pub input: &'c mut Input,
    pub config: &'d mut UserConfig,
}

pub trait AppState {
    fn process(&mut self, args: StateArgs);

    fn next_state(self: Box<Self>) -> Box<dyn AppState>;
}
