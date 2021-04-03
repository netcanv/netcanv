use std::rc::Rc;
use std::time::Duration;

use skulpin::skia_safe::*;
use skulpin::skia_safe::paint as skpaint;

use crate::app::*;
use crate::assets::*;
use crate::paint_canvas::*;
use crate::ui::*;
use crate::util::*;
use crate::net::{Message, Peer, Timer};

#[derive(PartialEq, Eq)]
enum PaintMode {
    None,
    Paint,
    Erase,
}

pub struct State {
    assets: Assets,

    ui: Ui,
    paint_canvas: PaintCanvas<'static>,
    peer: Peer,
    update_timer: Timer,

    paint_mode: PaintMode,
    paint_color: Color4f,
    brush_size_slider: Slider,
    stroke_buffer: Vec<StrokePoint>,

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

macro_rules! ok_or_log {
    ($exp:expr) => {
        match $exp {
            Ok(x) => x,
            Err(e) => eprintln!("{}", e),
        }
    };
}

impl State {

    const BAR_SIZE: f32 = 32.0;
    const TIME_PER_UPDATE: Duration = Duration::from_millis(50);

    pub fn new(assets: Assets, peer: Peer) -> Self {
        Self {
            assets,

            ui: Ui::new(),
            paint_canvas: PaintCanvas::new(),
            peer,
            update_timer: Timer::new(Self::TIME_PER_UPDATE),

            paint_mode: PaintMode::None,
            paint_color: hex_color4f(COLOR_PALETTE[0]),
            brush_size_slider: Slider::new(4.0, 1.0, 64.0, SliderStep::Discrete(1.0)),
            stroke_buffer: Vec::new(),

            panning: false,
            pan: Vector::new(0.0, 0.0),
        }
    }

    fn fellow_stroke(canvas: &mut PaintCanvas, points: &[StrokePoint]) {
        if points.is_empty() { return; } // failsafe

        let mut from = points[0].point;
        for point in &points[1..] {
            canvas.stroke(from, point.point, &point.brush);
            from = point.point;
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
        let from = input.previous_mouse_position() - self.pan;
        let to = input.mouse_position() - self.pan;
        loop { // give me back my labelled blocks
            let brush = match self.paint_mode {
                PaintMode::None => break,
                PaintMode::Paint =>
                    Brush::Draw {
                        color: self.paint_color.clone(),
                        stroke_width: brush_size,
                    },
                PaintMode::Erase =>
                    Brush::Erase {
                        stroke_width: brush_size,
                    },
            };
            self.paint_canvas.stroke(from, to, &brush);
            if self.stroke_buffer.is_empty() || to != self.stroke_buffer.last().unwrap().point {
                if self.stroke_buffer.is_empty() {
                    self.stroke_buffer.push(StrokePoint {
                        point: from,
                        brush: brush.clone(),
                    });
                }
                self.stroke_buffer.push(StrokePoint {
                    point: to,
                    brush,
                });
            }
            break;
        }

        for _ in self.update_timer.tick() {
            if from != to {
                ok_or_log!(self.peer.send_cursor(to, brush_size));
            }
            if !self.stroke_buffer.is_empty() {
                println!("sending {} points", self.stroke_buffer.len());
                ok_or_log!(self.peer.send_stroke(self.stroke_buffer.drain(..)));
            }
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

            let mut outline = Paint::new(Color4f::from(Color::WHITE.with_a(192)), None);
            outline.set_anti_alias(true);
            outline.set_style(skpaint::Style::Stroke);
            outline.set_blend_mode(BlendMode::Difference);

            paint_canvas.draw_to(canvas);
            for (_, mate) in self.peer.mates() {
                canvas.draw_circle(mate.cursor, mate.brush_size * 0.5, &outline);
            }

            canvas.restore();

            let mouse = self.ui.mouse_position(&input);
            canvas.draw_circle(mouse, self.brush_size_slider.value() * 0.5, &outline);
        });
        if self.panning {
            let position = format!("{}, {}", f32::floor(self.pan.x / 256.0), f32::floor(self.pan.y / 256.0));
            self.ui.push_group(self.ui.size(), Layout::Freeform);
            self.ui.pad((32.0, 32.0));
            self.ui.push_group((72.0, 32.0), Layout::Freeform);
            self.ui.fill(canvas, Color::BLACK.with_a(128));
            self.ui.text(canvas, &position, Color::WHITE, (AlignH::Center, AlignV::Middle));
            self.ui.pop_group();
            self.ui.pop_group();
        }

        self.ui.pop_group();
    }

    fn process_bar(&mut self, canvas: &mut Canvas, input: &mut Input) {
        if self.paint_mode != PaintMode::None {
            input.lock_mouse_buttons();
        }

        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Horizontal);
        self.ui.fill(canvas, self.assets.colors.panel);
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
        self.ui.text(canvas, "Brush size", self.assets.colors.text, (AlignH::Center, AlignV::Middle));
        self.ui.pop_group();

        self.ui.space(8.0);
        self.brush_size_slider.process(&mut self.ui, canvas, input, SliderArgs {
            width: 192.0,
            color: self.assets.colors.slider,
        });
        self.ui.space(8.0);

        let brush_size_string = self.brush_size_slider.value().to_string();
        self.ui.push_group((self.ui.height(), self.ui.height()), Layout::Freeform);
        self.ui.set_font(self.assets.sans_bold.clone());
        self.ui.text(canvas, &brush_size_string, self.assets.colors.text, (AlignH::Center, AlignV::Middle));
        self.ui.pop_group();

        //
        // right side
        //

        // room ID

        if self.peer.is_host() {
            self.ui.push_group((self.ui.remaining_width(), self.ui.height()), Layout::Freeform);
            self.ui.push_group((128.0, self.ui.height()), Layout::Horizontal);
            self.ui.align((AlignH::Right, AlignV::Top));

            // "Room ID" text
            self.ui.push_group((64.0, self.ui.height()), Layout::Freeform);
            self.ui.text(canvas, "Room ID", self.assets.colors.text, (AlignH::Center, AlignV::Middle));
            self.ui.pop_group();

            // the room ID itself
            let id_text = format!("{:04}", self.peer.room_id().unwrap());
            self.ui.push_group((64.0, self.ui.height()), Layout::Freeform);
            self.ui.set_font(self.assets.sans_bold.clone());
            self.ui.text(canvas, &id_text, self.assets.colors.text, (AlignH::Center, AlignV::Middle));
            self.ui.pop_group();

            self.ui.pop_group();
            self.ui.pop_group();
        }

        self.ui.pop_group();

        input.unlock_mouse_buttons();

    }

}

impl AppState for State {

    fn process(
        &mut self,
        StateArgs {
            canvas,
            coordinate_system_helper,
            input,
        }: StateArgs,
    ) {
        canvas.clear(Color::WHITE);

        // network
        match self.peer.tick() {
            Ok(messages) => for message in messages {
                match message {
                    Message::Stroke(points) => Self::fellow_stroke(&mut self.paint_canvas, &points),
                    x => eprintln!("{:?}", x),
                }
            },
            Err(error) => {
                eprintln!("{}", error);
            },
        }

        // UI setup
        self.ui.begin(get_window_size(&coordinate_system_helper), Layout::Vertical);
        self.ui.set_font(self.assets.sans.clone());
        self.ui.set_font_size(14.0);

        // canvas
        self.process_canvas(canvas, input);

        // bar
        self.process_bar(canvas, input);
    }

    fn next_state(self: Box<Self>) -> Box<dyn AppState> {
        self
    }

}
