use std::collections::{HashSet, VecDeque};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use native_dialog::FileDialog;
use skulpin::skia_safe::paint as skpaint;
use skulpin::skia_safe::*;

use crate::app::*;
use crate::assets::*;
use crate::config::UserConfig;
use crate::net::{Message, Peer, Timer};
use crate::paint_canvas::*;
use crate::ui::*;
use crate::util::*;
use crate::viewport::Viewport;

#[derive(PartialEq, Eq)]
enum PaintMode {
    None,
    Paint,
    Erase,
}

type Log = Vec<(String, Instant)>;

struct Tip {
    text: String,
    created: Instant,
    visible_duration: Duration,
}

pub struct State {
    assets: Assets,
    config: UserConfig,

    ui: Ui,
    paint_canvas: PaintCanvas,
    peer: Peer,
    update_timer: Timer,

    paint_mode: PaintMode,
    paint_color: Color4f,
    brush_size_slider: Slider,
    stroke_buffer: Vec<StrokePoint>,

    server_side_chunks: HashSet<(i32, i32)>,
    requested_chunks: HashSet<(i32, i32)>,
    downloaded_chunks: HashSet<(i32, i32)>,
    needed_chunks: HashSet<(i32, i32)>,
    deferred_message_queue: VecDeque<Message>,

    load_from_file: Option<PathBuf>,
    save_to_file: Option<PathBuf>,
    last_autosave: Instant,

    error: Option<String>,
    log: Log,
    tip: Tip,

    panning: bool,
    viewport: Viewport,
}

const COLOR_PALETTE: &'static [u32] = &[
    0x100820ff, 0xff003eff, 0xff7b00ff, 0xffff00ff, 0x2dd70eff, 0x03cbfbff, 0x0868ebff, 0xa315d7ff, 0xffffffff,
];

macro_rules! log {
    ($log:expr, $($arg:tt)*) => {
        $log.push((format!($($arg)*), Instant::now()))
    };
}

macro_rules! ok_or_log {
    ($log:expr, $exp:expr) => {
        match $exp {
            Ok(x) => x,
            Err(e) => log!($log, "{}", e),
        }
    };
}

impl State {
    // TODO: config
    const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(3 * 60);
    const BAR_SIZE: f32 = 32.0;
    pub const TIME_PER_UPDATE: Duration = Duration::from_millis(50);

    pub fn new(assets: Assets, config: UserConfig, peer: Peer, image_path: Option<PathBuf>) -> Self {
        let mut this = Self {
            assets,
            config,

            ui: Ui::new(),
            paint_canvas: PaintCanvas::new(),
            peer,
            update_timer: Timer::new(Self::TIME_PER_UPDATE),

            paint_mode: PaintMode::None,
            paint_color: hex_color4f(COLOR_PALETTE[0]),
            brush_size_slider: Slider::new(4.0, 1.0, 64.0, SliderStep::Discrete(1.0)),
            stroke_buffer: Vec::new(),

            server_side_chunks: HashSet::new(),
            requested_chunks: HashSet::new(),
            downloaded_chunks: HashSet::new(),
            needed_chunks: HashSet::new(),
            deferred_message_queue: VecDeque::new(),

            load_from_file: image_path,
            save_to_file: None,
            last_autosave: Instant::now(),

            error: None,
            log: Log::new(),
            tip: Tip {
                text: "".into(),
                created: Instant::now(),
                visible_duration: Default::default(),
            },

            panning: false,
            viewport: Viewport::new(),
        };
        if this.peer.is_host() {
            log!(this.log, "Welcome to your room!");
            log!(
                this.log,
                "To invite friends, send them the room ID shown in the bottom right corner of your screen."
            );
        }
        this
    }

    fn show_tip(&mut self, text: &str, duration: Duration) {
        self.tip = Tip {
            text: text.into(),
            created: Instant::now(),
            visible_duration: duration,
        };
    }

    fn fellow_stroke(canvas: &mut Canvas, paint_canvas: &mut PaintCanvas, points: &[StrokePoint]) {
        if points.is_empty() {
            return
        } // failsafe

        let mut from = points[0].point;
        let first_index = if points.len() > 1 { 1 } else { 0 };
        for point in &points[first_index..] {
            paint_canvas.stroke(canvas, from, point.point, &point.brush);
            from = point.point;
        }
    }

    fn canvas_data(
        log: &mut Log,
        canvas: &mut Canvas,
        paint_canvas: &mut PaintCanvas,
        chunk_position: (i32, i32),
        png_image: &[u8],
    ) {
        ok_or_log!(log, paint_canvas.decode_network_data(canvas, chunk_position, png_image));
    }

    fn process_log(&mut self, canvas: &mut Canvas) {
        self.log
            .retain(|(_, time_created)| time_created.elapsed() < Duration::from_secs(5));
        self.ui.draw_on_canvas(canvas, |canvas| {
            let mut paint = Paint::new(Color4f::from(Color::WHITE.with_a(240)), None);
            paint.set_blend_mode(BlendMode::Difference);
            let mut y = self.ui.height() - (self.log.len() as f32 - 1.0) * 16.0 - 8.0;
            for (entry, _) in &self.log {
                canvas.draw_str(&entry, (8.0, y), &self.assets.sans.borrow(), &paint);
                y += 16.0;
            }
        });
    }

    fn process_canvas(&mut self, canvas: &mut Canvas, input: &Input) {
        self.ui
            .push_group((self.ui.width(), self.ui.height() - Self::BAR_SIZE), Layout::Freeform);
        let canvas_size = self.ui.size();

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
        let from = self
            .viewport
            .to_viewport_space(input.previous_mouse_position(), canvas_size);
        let mouse_position = input.mouse_position();
        let to = self.viewport.to_viewport_space(mouse_position, canvas_size);
        loop {
            // give me back my labelled blocks
            let brush = match self.paint_mode {
                PaintMode::None => break,
                PaintMode::Paint => Brush::Draw {
                    color: self.paint_color.clone(),
                    stroke_width: brush_size,
                },
                PaintMode::Erase => Brush::Erase {
                    stroke_width: brush_size,
                },
            };
            self.paint_canvas.stroke(canvas, from, to, &brush);
            if self.stroke_buffer.is_empty() {
                self.stroke_buffer.push(StrokePoint {
                    point: from,
                    brush: brush.clone(),
                });
            } else if to != self.stroke_buffer.last().unwrap().point {
                self.stroke_buffer.push(StrokePoint { point: to, brush });
            }
            break
        }

        // panning and zooming

        if self.ui.has_mouse(input) && input.mouse_button_just_pressed(MouseButton::Middle) {
            self.panning = true;
        }
        if input.mouse_button_just_released(MouseButton::Middle) {
            self.panning = false;
        }

        if self.panning {
            let delta_pan = input.previous_mouse_position() - input.mouse_position();
            self.viewport.pan_around(delta_pan);
            let pan = self.viewport.pan();
            let position = format!("{}, {}", (pan.x / 256.0).floor(), (pan.y / 256.0).floor());
            self.show_tip(&position, Duration::from_millis(100));
        }
        if input.mouse_scroll().y != 0.0 {
            self.viewport.zoom_in(input.mouse_scroll().y);
            self.show_tip(&format!("{:.0}%", self.viewport.zoom() * 100.0), Duration::from_secs(3));
        }

        //
        // rendering
        //

        let paint_canvas = &self.paint_canvas;
        self.ui.draw_on_canvas(canvas, |canvas| {
            canvas.save();
            canvas.translate((self.ui.width() / 2.0, self.ui.height() / 2.0));
            canvas.scale((self.viewport.zoom(), self.viewport.zoom()));
            canvas.translate(-self.viewport.pan());

            let mut paint = Paint::new(Color4f::from(Color::WHITE.with_a(240)), None);
            paint.set_anti_alias(true);
            paint.set_blend_mode(BlendMode::Difference);

            paint_canvas.draw_to(canvas, &self.viewport, canvas_size);

            canvas.restore();

            for (_, mate) in self.peer.mates() {
                let cursor = self.viewport.to_screen_space(mate.lerp_cursor(), canvas_size);
                let brush_radius = mate.brush_size * self.viewport.zoom() * 0.5;
                let text_position = cursor + Point::new(brush_radius, brush_radius) + Point::new(0.0, 14.0);
                paint.set_style(skpaint::Style::Fill);
                canvas.draw_str(&mate.nickname, text_position, &self.assets.sans.borrow(), &paint);
                paint.set_style(skpaint::Style::Stroke);
                canvas.draw_circle(cursor, brush_radius, &paint);
            }

            let zoomed_brush_size = brush_size * self.viewport.zoom();
            paint.set_style(skpaint::Style::Stroke);
            canvas.draw_circle(mouse_position, zoomed_brush_size * 0.5, &paint);
        });
        if self.tip.created.elapsed() < self.tip.visible_duration {
            self.ui.push_group(self.ui.size(), Layout::Freeform);
            self.ui.pad((32.0, 32.0));
            self.ui.push_group((72.0, 32.0), Layout::Freeform);
            self.ui.fill(canvas, Color::BLACK.with_a(192));
            self.ui
                .text(canvas, &self.tip.text, Color::WHITE, (AlignH::Center, AlignV::Middle));
            self.ui.pop_group();
            self.ui.pop_group();
        }

        self.process_log(canvas);

        self.ui.pop_group();

        //
        // networking
        //

        for _ in self.update_timer.tick() {
            // mouse / drawing
            if input.previous_mouse_position() != input.mouse_position() {
                ok_or_log!(self.log, self.peer.send_cursor(to, brush_size));
            }
            if !self.stroke_buffer.is_empty() {
                ok_or_log!(self.log, self.peer.send_stroke(self.stroke_buffer.drain(..)));
            }
            // chunk downloading
            if self.save_to_file.is_some() {
                if self.requested_chunks.len() < self.server_side_chunks.len() {
                    self.needed_chunks
                        .extend(self.server_side_chunks.difference(&self.requested_chunks));
                } else if self.downloaded_chunks.len() == self.server_side_chunks.len() {
                    ok_or_log!(
                        self.log,
                        self.paint_canvas.save(Some(&self.save_to_file.as_ref().unwrap()))
                    );
                    self.last_autosave = Instant::now();
                    self.save_to_file = None;
                }
            } else {
                for chunk_position in self.viewport.visible_tiles(Chunk::SIZE, canvas_size) {
                    if self.server_side_chunks.contains(&chunk_position) &&
                        !self.requested_chunks.contains(&chunk_position)
                    {
                        self.needed_chunks.insert(chunk_position);
                    }
                }
            }
        }
    }

    fn process_bar(&mut self, canvas: &mut Canvas, input: &mut Input) {
        if self.paint_mode != PaintMode::None {
            input.lock_mouse_buttons();
        }

        self.ui
            .push_group((self.ui.width(), self.ui.remaining_height()), Layout::Horizontal);
        self.ui.fill(canvas, self.assets.colors.panel);
        self.ui.pad((16.0, 0.0));

        // palette

        for hex_color in COLOR_PALETTE {
            let color = hex_color4f(*hex_color);
            self.ui.push_group((16.0, self.ui.height()), Layout::Freeform);
            let y_offset = self.ui.height() *
                if self.paint_color == color {
                    0.5
                } else if self.ui.has_mouse(&input) {
                    0.7
                } else {
                    0.8
                };
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
        self.ui.text(
            canvas,
            "Brush size",
            self.assets.colors.text,
            (AlignH::Center, AlignV::Middle),
        );
        self.ui.pop_group();

        self.ui.space(8.0);
        self.brush_size_slider.process(&mut self.ui, canvas, input, SliderArgs {
            width: 192.0,
            color: self.assets.colors.slider,
        });
        self.ui.space(8.0);

        let brush_size_string = self.brush_size_slider.value().to_string();
        self.ui
            .push_group((self.ui.height(), self.ui.height()), Layout::Freeform);
        self.ui.set_font(self.assets.sans_bold.clone());
        self.ui.text(
            canvas,
            &brush_size_string,
            self.assets.colors.text,
            (AlignH::Center, AlignV::Middle),
        );
        self.ui.pop_group();

        //
        // right side
        //

        // room ID

        self.ui
            .push_group((self.ui.remaining_width(), self.ui.height()), Layout::HorizontalRev);
        // note that the elements go from right to left
        // the save button
        if Button::with_icon(
            &mut self.ui,
            canvas,
            input,
            ButtonArgs {
                height: 32.0,
                colors: &self.assets.colors.tool_button,
            },
            &self.assets.icons.file.save,
        )
        .clicked()
        {
            match FileDialog::new()
                .set_filename("canvas.png")
                .add_filter("PNG image", &["png"])
                .add_filter("NetCanv canvas", &["netcanv", "toml"])
                .show_save_single_file()
            {
                Ok(Some(path)) => {
                    self.save_to_file = Some(path);
                },
                Err(error) => log!(self.log, "Error while selecting file: {}", error),
                _ => (),
            }
        }
        if self.peer.is_host() {
            // the room ID itself
            let id_text = format!("{:04}", self.peer.room_id().unwrap());
            self.ui.push_group((64.0, self.ui.height()), Layout::Freeform);
            self.ui.set_font(self.assets.sans_bold.clone());
            self.ui.text(
                canvas,
                &id_text,
                self.assets.colors.text,
                (AlignH::Center, AlignV::Middle),
            );
            self.ui.pop_group();

            // "Room ID" text
            self.ui.push_group((64.0, self.ui.height()), Layout::Freeform);
            self.ui.text(
                canvas,
                "Room ID",
                self.assets.colors.text,
                (AlignH::Center, AlignV::Middle),
            );
            self.ui.pop_group();
        }
        self.ui.pop_group();

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

        // loading from file

        if self.load_from_file.is_some() {
            ok_or_log!(
                self.log,
                self.paint_canvas.load(canvas, &self.load_from_file.take().unwrap())
            );
        }

        // autosaving

        if self.paint_canvas.filename().is_some() && self.last_autosave.elapsed() > Self::AUTOSAVE_INTERVAL {
            eprintln!("autosaving chunks");
            ok_or_log!(self.log, self.paint_canvas.save(None));
            eprintln!("autosave complete");
            self.last_autosave = Instant::now();
        }

        // network

        match self.peer.tick() {
            Ok(messages) =>
                for message in messages {
                    match message {
                        Message::Error(error) => self.error = Some(error),
                        Message::Connected => unimplemented!(
                            "Message::Connected shouldn't be generated after connecting to the matchmaker"
                        ),
                        Message::Left(nickname) => log!(self.log, "{} left the room", nickname),
                        Message::Stroke(points) => Self::fellow_stroke(canvas, &mut self.paint_canvas, &points),
                        Message::ChunkPositions(mut positions) => {
                            eprintln!("received {} chunk positions", positions.len());
                            eprintln!("the positions are: {:?}", &positions);
                            self.server_side_chunks = positions.drain(..).collect();
                        },
                        Message::Chunks(chunks) => {
                            eprintln!("received {} chunks", chunks.len());
                            for (chunk_position, png_data) in chunks {
                                Self::canvas_data(
                                    &mut self.log,
                                    canvas,
                                    &mut self.paint_canvas,
                                    chunk_position,
                                    &png_data,
                                );
                                self.downloaded_chunks.insert(chunk_position);
                            }
                        },
                        message => self.deferred_message_queue.push_back(message),
                    }
                },
            Err(error) => {
                self.error = Some(format!("{}", error));
            },
        }

        for message in self.deferred_message_queue.drain(..) {
            match message {
                Message::Joined(nickname, addr) => {
                    log!(self.log, "{} joined the room", nickname);
                    if let Some(addr) = addr {
                        let positions = self.paint_canvas.chunk_positions();
                        ok_or_log!(self.log, self.peer.send_chunk_positions(addr, positions));
                    }
                },
                Message::GetChunks(addr, positions) => {
                    eprintln!("got request from {} for {} chunks", addr, positions.len());
                    let paint_canvas = &mut self.paint_canvas;
                    for (i, chunks) in positions.chunks(32).enumerate() {
                        eprintln!("  sending packet #{} containing {} chunks", i, chunks.len());
                        let packet: Vec<((i32, i32), Vec<u8>)> = chunks
                            .iter()
                            .filter_map(|position| {
                                paint_canvas
                                    .network_data(*position)
                                    .map(|slice| (*position, Vec::from(slice)))
                            })
                            .collect();
                        ok_or_log!(self.log, self.peer.send_chunks(addr, packet));
                    }
                    eprintln!("  all packets sent");
                },
                _ => unreachable!("unhandled peer message type"),
            }
        }

        if self.needed_chunks.len() > 0 {
            for chunk in &self.needed_chunks {
                self.requested_chunks.insert(*chunk);
            }
            ok_or_log!(
                self.log,
                self.peer.download_chunks(self.needed_chunks.drain().collect())
            );
        }

        // UI setup
        self.ui
            .begin(get_window_size(&coordinate_system_helper), Layout::Vertical);
        self.ui.set_font(self.assets.sans.clone());
        self.ui.set_font_size(14.0);

        // canvas
        self.process_canvas(canvas, input);

        // bar
        self.process_bar(canvas, input);
    }

    fn next_state(self: Box<Self>) -> Box<dyn AppState> {
        if let Some(error) = self.error {
            Box::new(lobby::State::new(self.assets, self.config, Some(&error)))
        } else {
            self
        }
    }
}
