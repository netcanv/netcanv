use skulpin::*;
use skulpin::app::{AppHandler, AppUpdateArgs, AppDrawArgs, AppError, MouseButton};
use skulpin::skia_safe::*;

use crate::paint_canvas::*;

pub struct NetCanv<'a> {
    pub font_sans: Font,
    pub font_sans_bold: Font,
    pub paint_canvas: PaintCanvas<'a>,

    previous_mouse: (f64, f64),
}

const SANS_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

const DEFAULT_CANVAS_SIZE: (u32, u32) = (1024, 600);

impl NetCanv<'_> {

    pub fn new() -> Self {
        let sans_typeface = Typeface::from_data(Data::new_copy(SANS_TTF), None).unwrap();
        let sans_bold_typeface = Typeface::from_data(Data::new_copy(SANS_BOLD_TTF), None).unwrap();
        NetCanv {
            font_sans: Font::new(sans_typeface, 15.0),
            font_sans_bold: Font::new(sans_bold_typeface, 15.0),
            paint_canvas: PaintCanvas::new(DEFAULT_CANVAS_SIZE),
            previous_mouse: (0.0, 0.0),
        }
    }

}

impl AppHandler for NetCanv<'_> {

    fn update(
        &mut self,
        AppUpdateArgs {
            app_control: _,
            input_state,
            time_state: _,
        }: AppUpdateArgs
    ) {
        let mouse: (f64, f64) = input_state.mouse_position().into();

        if input_state.is_mouse_down(MouseButton::Left) {
            self.paint_canvas.stroke(
                (self.previous_mouse.0 as f32, self.previous_mouse.1 as f32),
                (mouse.0 as f32, mouse.1 as f32),
                &Brush::Draw {
                    color: Color4f::from(Color::BLACK),
                    stroke_width: 4.0,
                },
            );
        } else if input_state.is_mouse_down(MouseButton::Right) {
            self.paint_canvas.stroke(
                (self.previous_mouse.0 as f32, self.previous_mouse.1 as f32),
                (mouse.0 as f32, mouse.1 as f32),
                &Brush::Erase {
                    stroke_width: 8.0,
                },
            );
        }

        self.previous_mouse = mouse;
    }

    fn draw(&mut self, args: AppDrawArgs) {
        let canvas = args.canvas;
        canvas.clear(Color::WHITE);

        canvas.draw_bitmap(
            &self.paint_canvas,
            (0.0, 0.0),
            None,
        );
    }

    fn fatal_error(&mut self, error: &AppError) {
        println!("Fatal error: {}", error);
    }

}
