//! A fairly simplistic text field implementation.

use skulpin::skia_safe::*;

use crate::ui::*;

/// A text field's state.
pub struct TextField {
    text: Vec<char>,
    text_utf8: String,
    focused: bool,
    blink_start: f32,
    cursor_pos: usize,
}

/// A text field's color scheme.
#[derive(Clone)]
pub struct TextFieldColors {
    pub outline: Color,
    pub outline_focus: Color,
    pub fill: Color,
    pub text: Color,
    pub text_hint: Color,
    pub label: Color,
}

/// Processing arguments for a text field.
#[derive(Clone, Copy)]
pub struct TextFieldArgs<'a, 'b> {
    pub width: f32,
    pub colors: &'a TextFieldColors,
    pub hint: Option<&'b str>,
}

impl TextField {
    /// The backspace character.
    const BACKSPACE: char = '\x08';
    /// The blinking period of the caret.
    const BLINK_PERIOD: f32 = 1.0;
    const HALF_BLINK: f32 = Self::BLINK_PERIOD / 2.0;

    /// Creates a new text field, with the optionally provided initial text.
    pub fn new(initial_text: Option<&str>) -> Self {
        let text_utf8: String = initial_text.unwrap_or("").into();
        let length = text_utf8.len();

        Self {
            text: text_utf8.chars().collect(),
            text_utf8,
            focused: false,
            blink_start: 0.0,
            cursor_pos: length,
        }
    }

    /// Updates the text field's UTF-8 string.
    fn update_utf8(&mut self) {
        self.text_utf8 = self.text.iter().collect();
    }

    /// Returns the height of a text field.
    pub fn height(ui: &Ui) -> f32 {
        f32::round(16.0 / 7.0 * ui.font_size())
    }

    /// Processes a text field.
    pub fn process(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        TextFieldArgs { width, colors, hint }: TextFieldArgs,
    ) {
        ui.push_group((width, Self::height(ui)), Layout::Freeform);

        // Rendering: box
        ui.draw_on_canvas(canvas, |canvas| {
            let mut paint = Paint::new(Color4f::from(colors.fill), None);
            paint.set_anti_alias(true);
            let mut rrect = RRect::new_rect_xy(&Rect::from_point_and_size((0.0, 0.0), ui.size()), 4.0, 4.0);
            canvas.draw_rrect(rrect, &paint);
            paint.set_color(if self.focused {
                colors.outline_focus
            } else {
                colors.outline
            });
            paint.set_style(paint::Style::Stroke);
            rrect.offset((0.5, 0.5));
            canvas.draw_rrect(rrect, &paint);
        });

        // Rendering: text
        ui.push_group(ui.size(), Layout::Freeform);
        ui.pad((16.0, 0.0));
        canvas.save();
        ui.clip(canvas);

        // Rendering: hint
        if hint.is_some() && self.text.len() == 0 {
            ui.text(canvas, hint.unwrap(), colors.text_hint, (AlignH::Left, AlignV::Middle));
        }

        ui.text(canvas, &self.text_utf8, colors.text, (AlignH::Left, AlignV::Middle));

        let text_advance = ui.text(canvas, &self.text_utf8, colors.text, (AlignH::Left, AlignV::Middle));

        if self.focused && (input.time_in_seconds() - self.blink_start) % Self::BLINK_PERIOD < Self::HALF_BLINK {
            ui.draw_on_canvas(canvas, |canvas| {
                let mut paint = Paint::new(Color4f::from(colors.text), None);
                paint.set_anti_alias(false);
                paint.set_style(paint::Style::Stroke);

                let curr_text: String = self.text[..self.cursor_pos].iter().collect();
                let cursor_x = ui.borrow_font().measure_str(curr_text, None).0;

                let x = cursor_x + 1.0;
                let y1 = Self::height(ui) * 0.2;
                let y2 = Self::height(ui) * 0.8;
                canvas.draw_line((x, y1), (x, y2), &paint);
            });
        }

        canvas.restore();
        ui.pop_group();

        // Process events
        self.process_events(ui, input);

        ui.pop_group();
    }

    /// Resets the text field's blink timer.
    fn reset_blink(&mut self, input: &Input) {
        self.blink_start = input.time_in_seconds();
    }

    /// Appends a character to the cursor position.
    fn append(&mut self, ch: char) {
        self.text.insert(self.cursor_pos, ch);

        self.cursor_pos += 1;

        self.update_utf8();
    }

    /// Removes a character at cursor position.
    fn backspace(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.text.remove(self.cursor_pos);
        }

        self.update_utf8();
    }

    /// Removes character after cursor
    fn delete(&mut self) {
        if self.text.len() > 0 {
            self.text.remove(self.cursor_pos);
        }

        self.update_utf8();
    }

    /// Processes input events.
    fn process_events(&mut self, ui: &Ui, input: &Input) {
        if input.mouse_button_just_pressed(MouseButton::Left) {
            self.focused = ui.has_mouse(input);
            if self.focused {
                self.reset_blink(input);
            }
        }
        if self.focused {
            if !input.characters_typed().is_empty() {
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::Left) {
                if self.cursor_pos != 0 {
                    self.reset_blink(input);
                    self.cursor_pos -= 1;
                }
            }

            if input.key_just_typed(VirtualKeyCode::Right) {
                if self.cursor_pos < self.text.len() {
                    self.reset_blink(input);
                    self.cursor_pos += 1;
                }
            }

            if input.key_just_typed(VirtualKeyCode::Delete) {
                self.delete();
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::Home) {
                self.cursor_pos = 0;
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::End) {
                self.cursor_pos = self.text.len();
                self.reset_blink(input);
            }

            for ch in input.characters_typed() {
                match *ch {
                    _ if !ch.is_control() => self.append(*ch),
                    Self::BACKSPACE => self.backspace(),
                    _ => (),
                }
            }
        }
    }

    /// Returns the height of a labelled text field.
    pub fn labelled_height(ui: &Ui) -> f32 {
        16.0 + TextField::height(ui)
    }

    /// Processes a text field with an extra label above it.
    pub fn with_label(&mut self, ui: &mut Ui, canvas: &mut Canvas, input: &Input, label: &str, args: TextFieldArgs) {
        ui.push_group((args.width, Self::labelled_height(ui)), Layout::Vertical);

        // label
        ui.push_group((args.width, 16.0), Layout::Freeform);
        ui.text(canvas, label, args.colors.label, (AlignH::Left, AlignV::Top));
        ui.pop_group();

        // field
        self.process(ui, canvas, input, args);

        ui.pop_group();
    }

    /// Returns the text in the text field.
    pub fn text<'a>(&'a self) -> &'a str {
        &self.text_utf8
    }
}

impl Focus for TextField {
    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focus(&mut self, focused: bool) {
        self.focused = focused;
    }
}
