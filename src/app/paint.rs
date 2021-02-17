use std::rc::Rc;

use skulpin::skia_safe::*;
use skulpin::skia_safe::paint as skpaint;

use crate::app::*;
use crate::assets::*;
use crate::paint_canvas::*;
use crate::ui::*;
use crate::util::*;

#[derive(PartialEq, Eq)]
enum PaintMode {
    None,
    Paint,
    Erase,
}

pub struct State<'a> {
    assets: Assets,

    ui: Ui,
    paint_canvas: PaintCanvas<'a>,

    paint_mode: PaintMode,
    paint_color: Color4f,
    brush_size_slider: Slider,

    panning: bool,
    pan: Vector,
}

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

impl<'a> State<'a> {

    const BAR_SIZE: f32 = 32.0;

    pub fn new(assets: Assets) -> Self {
        Self {
            assets,

            ui: Ui::new(),
            paint_canvas: PaintCanvas::new(),

            paint_mode: PaintMode::None,
            paint_color: hex_color4f(COLOR_PALETTE[0]),
            brush_size_slider: Slider::new(4.0, 1.0, 64.0, SliderStep::Discrete(1.0)),

            panning: false,
            pan: Vector::new(0.0, 0.0),
        }
    }

    fn process_canvas(&mut self, canvas: &mut Canvas, input: &Input) {
        self.ui.push_group((self.ui.width(), self.ui.height() - Self::BAR_SIZE), Layout::Freeform);

        //
        // input
        //

        // drawing

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
                    input.previous_mouse_position() - self.pan,
                    input.mouse_position() - self.pan,
                    &Brush::Draw {
                        color: self.paint_color.clone(),
                        stroke_width: brush_size,
                    },
                ),
            PaintMode::Erase =>
                self.paint_canvas.stroke(
                    input.previous_mouse_position() - self.pan,
                    input.mouse_position() - self.pan,
                    &Brush::Erase {
                        stroke_width: brush_size,
                    },
                ),
        }

        // panning

        if self.ui.has_mouse(input) && input.mouse_button_just_pressed(MouseButton::Middle) {
            self.panning = true;
        }
        if input.mouse_button_just_released(MouseButton::Middle) {
            self.panning = false;
        }

        if self.panning {
            let delta_pan = input.mouse_position() - input.previous_mouse_position();
            self.pan.offset(delta_pan);
        }

        //
        // rendering
        //

        let paint_canvas = &self.paint_canvas;
        self.ui.draw_on_canvas(canvas, |canvas| {
            canvas.save();
            canvas.translate(self.pan);
            paint_canvas.draw_to(canvas);
            canvas.restore();

            let mouse = self.ui.mouse_position(&input);
            let mut outline = Paint::new(Color4f::from(Color::WHITE.with_a(192)), None);
            outline.set_anti_alias(true);
            outline.set_style(skpaint::Style::Stroke);
            outline.set_blend_mode(BlendMode::Difference);
            canvas.draw_circle(mouse, self.brush_size_slider.value() * 0.5, &outline);

        });
        if self.panning {
            let position = format!("{}, {}", f32::floor(self.pan.x / 256.0), f32::floor(self.pan.y / 256.0));
            self.ui.pad((32.0, 32.0));
            self.ui.push_group((72.0, 32.0), Layout::Freeform);
            self.ui.fill(canvas, Color::BLACK.with_a(128));
            self.ui.text(canvas, &position, Color::WHITE, (AlignH::Center, AlignV::Middle));
            self.ui.pop_group();
        }

        self.ui.pop_group();
    }

    fn process_bar(&mut self, canvas: &mut Canvas, input: &mut Input) {

        if self.paint_mode != PaintMode::None {
            input.lock_mouse_buttons();
        }

        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Horizontal);
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
            if self.ui.has_mouse(&input) && input.mouse_button_just_pressed(MouseButton::Left) {
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
        self.ui.set_font(self.assets.sans_bold.clone());
        self.ui.text(canvas, brush_size_string.as_str(), Color::BLACK, (AlignH::Center, AlignV::Middle));
        self.ui.pop_group();

        self.ui.pop_group();

        input.unlock_mouse_buttons();

    }

}

impl AppState for State<'_> {

    fn process(
        &mut self,
        StateArgs {
            canvas,
            coordinate_system_helper,
            input,
        }: StateArgs,
    ) -> Option<Box<dyn AppState>> {
        canvas.clear(Color::WHITE);

        let window_size: (f32, f32) = {
            let logical_size = coordinate_system_helper.window_logical_size();
            (logical_size.width as _, logical_size.height as _)
        };
        self.ui.begin(window_size, Layout::Vertical);
        self.ui.set_font(self.assets.sans.clone());
        self.ui.set_font_size(14.0);

        // canvas
        self.process_canvas(canvas, input);

        // bar
        self.process_bar(canvas, input);

        None
    }

}
