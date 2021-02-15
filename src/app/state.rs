use skulpin::CoordinateSystemHelper;
use skulpin::skia_safe::*;

use crate::ui::*;

pub struct StateArgs<'a, 'b, 'c> {
    pub canvas: &'a mut Canvas,
    pub coordinate_system_helper: &'b CoordinateSystemHelper,
    pub input: &'c mut Input,
}

pub trait AppState {
    // if this returns None, this means that the current app state should continue
    // if this returns Some(state), this means that the app state should be switched to `state`
    fn process(
        &mut self,
        args: StateArgs,
    ) -> Option<Box<dyn AppState>>;
}
