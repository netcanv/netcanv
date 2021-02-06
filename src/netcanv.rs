use skulpin::app::{AppHandler, AppUpdateArgs, AppDrawArgs, AppError, MouseButton};
use skulpin::skia_safe::*;

use crate::paint_canvas::*;
use crate::ui::*;
use crate::util::*;

#[derive(PartialEq, Eq)]
enum PaintMode {
    None,
    Paint,
    Erase,
}

pub struct NetCanv<'a> {
    pub font_sans: RcFont,
    pub font_sans_bold: RcFont,

    pub ui: Ui,
    pub paint_canvas: PaintCanvas<'a>,

    mouse_over_panel: bool,
    paint_mode: PaintMode,
    previous_mouse: Point,
    paint_color: Color4f,
    brush_size: f32,
}

const SANS_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

const DEFAULT_CANVAS_SIZE: (u32, u32) = (1024, 600);

const COLOR_PALETTE: &'static [u32] = &[
    0x100820ff,
    0xff003eff,
    0xff7b00ff,
    0xffff00ff,
    0x2dd70eff,
    0x03cbfbff,
    0x0868ebff,
    0xa315d7ff,
    0xffffffff,
];

impl NetCanv<'_> {

    pub fn new() -> Self {
        NetCanv {
            font_sans: new_rc_font(SANS_TTF, 15.0),
            font_sans_bold: new_rc_font(SANS_BOLD_TTF, 15.0),
            ui: Ui::new(),
            paint_canvas: PaintCanvas::new(DEFAULT_CANVAS_SIZE),
            mouse_over_panel: false,
            paint_mode: PaintMode::None,
            previous_mouse: Point::new(0.0, 0.0),
            paint_color: hex_color4f(COLOR_PALETTE[0]),
            brush_size: 4.0,
        }
    }

}

impl AppHandler for NetCanv<'_> {

    fn update(
        &mut self,
        AppUpdateArgs {
            app_control: _,
            input_state: input,
            time_state: _,
        }: AppUpdateArgs
    ) {
        let mouse = self.ui.abs_mouse_position(&input);

        if !self.mouse_over_panel {
            if input.is_mouse_just_down(MouseButton::Left) {
                self.paint_mode = PaintMode::Paint;
            } else if input.is_mouse_just_down(MouseButton::Right) {
                self.paint_mode = PaintMode::Erase;
            }
        }
        if input.is_mouse_just_up(MouseButton::Left) || input.is_mouse_just_up(MouseButton::Right) {
            self.paint_mode = PaintMode::None;
        }
        match self.paint_mode {
            PaintMode::None => (),
            PaintMode::Paint =>
                self.paint_canvas.stroke(
                    self.previous_mouse,
                    mouse,
                    &Brush::Draw {
                        color: self.paint_color.clone(),
                        stroke_width: self.brush_size,
                    },
                ),
            PaintMode::Erase =>
                self.paint_canvas.stroke(
                    self.previous_mouse,
                    mouse,
                    &Brush::Erase {
                        stroke_width: self.brush_size,
                    },
                ),
        }

        self.previous_mouse = mouse;
    }

    fn draw(
        &mut self,
        AppDrawArgs {
            app_control: _,
            input_state: input,
            time_state: _,
            canvas,
            coordinate_system_helper,
        }: AppDrawArgs
    ) {
        canvas.clear(Color::WHITE);

        let window_size: (f32, f32) = {
            let logical_size = coordinate_system_helper.window_logical_size();
            (logical_size.width as f32, logical_size.height as f32)
        };
        self.ui.begin(window_size, Layout::Vertical);
        self.ui.set_font(self.font_sans.clone());
        self.ui.set_font_size(14.0);

        // drawing area
        self.ui.push_group((self.ui.width(), self.ui.height() - 32.0), Layout::Freeform);
        self.ui.draw_on_canvas(canvas, |canvas| {
            canvas.draw_bitmap(
                &self.paint_canvas,
                (0.0, 0.0),
                None,
            );

            let mouse = self.ui.mouse_position(&input);
            let mut outline = Paint::new(Color4f::from(Color::WHITE.with_a(192)), None);
            outline.set_anti_alias(true);
            outline.set_style(paint::Style::Stroke);
            outline.set_blend_mode(BlendMode::Difference);
            canvas.draw_circle(mouse, self.brush_size * 0.5, &outline);
        });
        self.ui.pop_group();

        // bar

        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Horizontal);
        self.mouse_over_panel = self.ui.has_mouse(&input);
        self.ui.fill(canvas, Color4f::new(0.9, 0.9, 0.9, 1.0));
        self.ui.pad((16.0, 0.0));

        // palette
        for hex_color in COLOR_PALETTE {
            let color = hex_color4f(*hex_color);
            self.ui.push_group((16.0, self.ui.height()), Layout::Freeform);
            let y_offset = self.ui.height() *
                if self.paint_color == color { 0.5 }
                else if self.ui.has_mouse(&input) { 0.7 }
                else { 0.8 };
            if self.paint_mode == PaintMode::None &&
               self.ui.has_mouse(&input) &&
               input.is_mouse_down(MouseButton::Left) {
                self.paint_color = color.clone();
            }
            self.ui.draw_on_canvas(canvas, |canvas| {
                let paint = Paint::new(color, None);
                let rect = Rect::from_point_and_size((0.0, y_offset), self.ui.size());
                canvas.draw_rect(rect, &paint);
            });
            self.ui.pop_group();
        }
        self.ui.space(16.0);

        // brush size

        self.ui.push_group((80.0, self.ui.height()), Layout::Freeform);
        self.ui.text(canvas, "Brush size", Color::BLACK, (AlignH::Center, AlignV::Middle));
        self.ui.pop_group();

        self.ui.push_group((self.ui.height(), self.ui.height()), Layout::Freeform);
        self.ui.set_font(self.font_sans_bold.clone());
        self.ui.text(canvas, self.brush_size.to_string().as_str(), Color::BLACK, (AlignH::Center, AlignV::Middle));
        self.ui.pop_group();

        self.ui.pop_group();
    }

    fn fatal_error(&mut self, error: &AppError) {
        println!("Fatal error: {}", error);
    }

}
