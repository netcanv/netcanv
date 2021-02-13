use std::error::Error;

use skulpin::CoordinateSystemHelper;
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
    paint_color: Color4f,
    brush_size_slider: Slider,
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

    const BAR_SIZE: f32 = 32.0;

    pub fn new() -> Self {
        NetCanv {
            font_sans: new_rc_font(SANS_TTF, 15.0),
            font_sans_bold: new_rc_font(SANS_BOLD_TTF, 15.0),

            ui: Ui::new(),
            paint_canvas: PaintCanvas::new(DEFAULT_CANVAS_SIZE),

            mouse_over_panel: false,
            paint_mode: PaintMode::None,
            paint_color: hex_color4f(COLOR_PALETTE[0]),
            brush_size_slider: Slider::new(4.0, 1.0, 64.0, SliderStep::Discrete(1.0)),
        }
    }

    fn process_canvas(&mut self, canvas: &mut Canvas, input: &Input) {
        self.ui.push_group((self.ui.width(), self.ui.height() - Self::BAR_SIZE), Layout::Freeform);

        // input

        if self.ui.has_mouse(input) {
            if input.mouse_button_just_pressed(MouseButton::Left) {
                self.paint_mode = PaintMode::Paint;
            } else if input.mouse_button_just_pressed(MouseButton::Right) {
                self.paint_mode = PaintMode::Erase;
            }
        }
        if input.mouse_button_just_released(MouseButton::Left) || input.mouse_button_just_released(MouseButton::Right) {
            self.paint_mode = PaintMode::None;
        }

        let brush_size = self.brush_size_slider.value();
        match self.paint_mode {
            PaintMode::None => (),
            PaintMode::Paint =>
                self.paint_canvas.stroke(
                    input.previous_mouse_position(),
                    input.mouse_position(),
                    &Brush::Draw {
                        color: self.paint_color.clone(),
                        stroke_width: brush_size,
                    },
                ),
            PaintMode::Erase =>
                self.paint_canvas.stroke(
                    input.previous_mouse_position(),
                    input.mouse_position(),
                    &Brush::Erase {
                        stroke_width: brush_size,
                    },
                ),
        }

        // rendering
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
            canvas.draw_circle(mouse, self.brush_size_slider.value() * 0.5, &outline);
        });

        self.ui.pop_group();
    }

    fn process_bar(&mut self, canvas: &mut Canvas, input: &Input) {

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
               input.mouse_button_is_down(MouseButton::Left) {
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

        self.ui.space(8.0);
        self.brush_size_slider.process(&mut self.ui, canvas, &input, 192.0, Color::BLACK);
        self.ui.space(8.0);

        let brush_size_string = self.brush_size_slider.value().to_string();
        self.ui.push_group((self.ui.height(), self.ui.height()), Layout::Freeform);
        self.ui.set_font(self.font_sans_bold.clone());
        self.ui.text(canvas, brush_size_string.as_str(), Color::BLACK, (AlignH::Center, AlignV::Middle));
        self.ui.pop_group();

        self.ui.pop_group();

    }

    pub fn process(
        &mut self,
        canvas: &mut Canvas,
        coordinate_system_helper: &CoordinateSystemHelper,
        input: &Input,
    ) -> Result<(), Box<dyn Error>> {
        canvas.clear(Color::WHITE);

        let window_size: (f32, f32) = {
            let logical_size = coordinate_system_helper.window_logical_size();
            (logical_size.width as _, logical_size.height as _)
        };
        self.ui.begin(window_size, Layout::Vertical);
        self.ui.set_font(self.font_sans.clone());
        self.ui.set_font_size(14.0);

        // canvas
        self.process_canvas(canvas, input);

        // bar
        self.process_bar(canvas, input);

        Ok(())
    }

}

// impl AppHandler for NetCanv<'_> {


//     fn draw(
//         &mut self,
//         AppDrawArgs {
//             app_control: _,
//             input_state: input,
//             time_state: _,
//             canvas,
//             coordinate_system_helper,
//         }: AppDrawArgs
//     ) {
//         canvas.clear(Color::WHITE);

//     }

//     fn fatal_error(&mut self, error: &AppError) {
//         println!("Fatal error: {}", error);
//     }

// }
