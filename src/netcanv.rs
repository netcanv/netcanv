use skulpin::*;
use skulpin::app::{AppBuilder, AppHandler, AppUpdateArgs, AppDrawArgs, AppError};
use skulpin::skia_safe::*;

use crate::paint_canvas::*;

pub struct NetCanv<'a> {
    pub font_sans: Font,
    pub font_sans_bold: Font,
    pub paint_canvas: PaintCanvas<'a>,
}

const SANS_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

impl NetCanv<'_> {

    pub fn new() -> Self {
        let sans_typeface = Typeface::from_data(Data::new_copy(SANS_TTF), None).unwrap();
        let sans_bold_typeface = Typeface::from_data(Data::new_copy(SANS_BOLD_TTF), None).unwrap();
        NetCanv {
            font_sans: Font::new(sans_typeface, 15.0),
            font_sans_bold: Font::new(sans_bold_typeface, 15.0),
            paint_canvas: PaintCanvas::new((1024, 600)),
        }
    }

}

impl AppHandler for NetCanv<'_> {

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

        canvas.draw_str(
            "Regular",
            Point::new(72.0, 32.0 + self.font_sans.size()),
            &self.font_sans,
            &black_fill,
        );

        canvas.draw_str(
            "Bold",
            Point::new(72.0, 48.0 + self.font_sans_bold.size()),
            &self.font_sans_bold,
            &black_fill,
        );
    }

    fn fatal_error(&mut self, error: &AppError) {
        println!("Fatal error: {}", error);
    }

}
