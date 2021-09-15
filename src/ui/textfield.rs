//! A fairly simplistic text field implementation.

use std::{borrow::BorrowMut, ops::Range};

use copypasta::{ClipboardContext, ClipboardProvider};
use skulpin::skia_safe::*;

use crate::ui::*;

/// A text field's state.
pub struct TextField {
    text: Vec<char>,
    text_utf8: String,
    focused: bool,
    blink_start: f32,

    cursor_position: usize,
    selection_start_cursor_position: usize,

    clipboard_context: ClipboardContext,
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
    pub selection: Color,
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

            cursor_position: length,
            selection_start_cursor_position: length,

            clipboard_context: ClipboardContext::new().unwrap(),
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

                let current_text: String = self.text[..self.cursor_position].iter().collect();
                let current_text_width = ui.borrow_font().measure_str(current_text, None).0;

                let x = current_text_width + 1.0;
                let y1 = Self::height(ui) * 0.2;
                let y2 = Self::height(ui) * 0.8;
                canvas.draw_line((x, y1), (x, y2), &paint);
            });
        }

        if self.cursor_position != self.selection_start_cursor_position {
            ui.draw_on_canvas(canvas, |canvas| {
                let mut paint = Paint::new(Color4f::from(colors.selection), None);
                paint.set_anti_alias(true);

                // Get all text from textfield start to current selection cursor position.
                // This will act as base position for selection.
                let text_to_selection_start: String = self.text[..self.selection_start_cursor_position].iter().collect();
                let text_to_selection_start_width = ui.borrow_font().measure_str(text_to_selection_start, None).0;

                // Get text left to right or right to left depending on end cursor position.
                let sel_text_width = if self.cursor_position < self.selection_start_cursor_position {
                    let selection_text: String = self.text[self.cursor_position..self.selection_start_cursor_position].iter().collect();
                    -(ui.borrow_font().measure_str(selection_text, None).0)
                } else {
                    let selection_text: String = self.text[self.selection_start_cursor_position..self.cursor_position].iter().collect();
                    ui.borrow_font().measure_str(selection_text, None).0
                };

                let rrect = RRect::new_rect_xy(
                    &Rect::from_point_and_size(
                        (text_to_selection_start_width.round(), (Self::height(ui) * 0.2).round()),
                        (sel_text_width.round(), (Self::height(ui) * 0.7).round()),
                    ),
                    0.0,
                    0.0,
                );
                canvas.draw_rrect(rrect, &paint);
            });
        }

        canvas.restore();
        ui.pop_group();

        // Process events
        self.process_events(ui, input);

        ui.pop_group();
    }

    /// Get selection length
    fn selection_length(&self) -> isize {
        (self.cursor_position as isize - self.selection_start_cursor_position as isize).abs()
    }

    // Get selection content
    fn selection_text(&self) -> String {
        if self.selection_length() == 0 {
            return String::new()
        }

        if self.cursor_position < self.selection_start_cursor_position {
            self.text[self.cursor_position..self.selection_start_cursor_position].iter().collect()
        } else {
            self.text[self.selection_start_cursor_position..self.cursor_position].iter().collect()
        }
    }

    // Set text
    fn set_text(&mut self, text: String) {
        self.text = text.chars().collect();
        self.update_utf8();

        self.cursor_position = self.text.len();
        self.selection_start_cursor_position = self.cursor_position;
    }

    /// Resets the text field's blink timer.
    fn reset_blink(&mut self, input: &Input) {
        self.blink_start = input.time_in_seconds();
    }

    /// Appends a character to the cursor position.
    /// Or replaces selection if any.
    fn append(&mut self, ch: char) {
        if self.selection_length() != 0 {
            if self.cursor_position < self.selection_start_cursor_position {
                self.text.splice(self.cursor_position..self.selection_start_cursor_position, vec![ch]);
                self.selection_start_cursor_position -= self.selection_length() as usize;
            } else {
                self.text.splice(self.selection_start_cursor_position..self.cursor_position, vec![ch]);
            }

            self.cursor_position = self.selection_start_cursor_position + 1;
            self.selection_start_cursor_position = self.cursor_position;
        } else {
            self.text.insert(self.cursor_position, ch);

            self.cursor_position += 1;
            self.selection_start_cursor_position = self.cursor_position;
        }

        self.update_utf8();
    }

    /// Removes a character at cursor position.
    /// Or removes selection if any.
    fn backspace(&mut self) {
        if self.selection_length() != 0 {
            self.delete();
        } else if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.selection_start_cursor_position -= 1;
            self.text.remove(self.cursor_position);
        }

        self.update_utf8();
    }

    /// Removes character after cursor position.
    /// Or selection if any.
    fn delete(&mut self) {
        if self.selection_length() != 0 {
            if self.cursor_position < self.selection_start_cursor_position {
                self.text.drain(self.cursor_position..self.selection_start_cursor_position);
                self.selection_start_cursor_position -= self.selection_length() as usize;
            } else {
                self.text.drain(self.selection_start_cursor_position..self.cursor_position);
            }

            self.cursor_position = self.selection_start_cursor_position;
        } else {
            if self.text.len() > 0 {
                self.text.remove(self.cursor_position);
            }
        }

        self.update_utf8();
    }

    fn key_ctrl_down(&self, input: &Input) -> bool {
        input.key_is_down(VirtualKeyCode::LControl) || input.key_is_down(VirtualKeyCode::RControl)
    }

    fn key_shift_down(&self, input: &Input) -> bool {
        input.key_is_down(VirtualKeyCode::LShift) || input.key_is_down(VirtualKeyCode::RShift)
    }

    fn handle_word_selection(&mut self, input: &Input, not_found_ws_value: usize, range: Range<usize>) {
        let mut ix = self.text[range.clone()].len();
        let mut found_ws = false;

        for char in self.text[range].iter().rev() {
            ix -= 1;

            if char.is_whitespace() {
                self.cursor_position = ix + 1;

                if !self.key_shift_down(input) {
                    self.selection_start_cursor_position = self.cursor_position;
                }

                found_ws = true;

                break
            }
        }

        if !found_ws {
            self.cursor_position = not_found_ws_value;

            if !self.key_shift_down(input) {
                self.selection_start_cursor_position = self.cursor_position;
            }
        }
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
                if self.cursor_position != 0 {
                    self.reset_blink(input);

                    self.cursor_position -= 1;
                }

                if !self.key_shift_down(input) {
                    self.selection_start_cursor_position = self.cursor_position;
                }

                if self.key_ctrl_down(input) {
                    self.handle_word_selection(input, 0, 0..self.cursor_position);
                }
            }

            if input.key_just_typed(VirtualKeyCode::Right) {
                if self.cursor_position < self.text.len() {
                    self.reset_blink(input);

                    self.cursor_position += 1;
                }

                if !(input.key_is_down(VirtualKeyCode::LShift) || input.key_is_down(VirtualKeyCode::RShift)) {
                    self.selection_start_cursor_position = self.cursor_position;
                }

                if self.key_ctrl_down(input) {
                    self.handle_word_selection(input, self.text.len(), self.cursor_position..self.text.len());
                }
            }

            if input.key_just_typed(VirtualKeyCode::Delete) {
                self.delete();
                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::Home) {
                self.cursor_position = 0;
                self.selection_start_cursor_position = self.cursor_position;

                self.reset_blink(input);
            }

            if input.key_just_typed(VirtualKeyCode::End) {
                self.cursor_position = self.text.len();
                self.selection_start_cursor_position = self.cursor_position;

                self.reset_blink(input);
            }

            if self.key_ctrl_down(input) {
                if input.key_just_typed(VirtualKeyCode::A) {
                    self.selection_start_cursor_position = 0;
                    self.cursor_position = self.text.len();
                }

                if input.key_just_typed(VirtualKeyCode::C) {
                    self.clipboard_context.set_contents(self.selection_text()).unwrap();
                }

                if input.key_just_typed(VirtualKeyCode::V) {
                    let content = self.clipboard_context.get_contents();

                    if content.is_ok() {
                        if self.selection_length() > 0 {
                            let mut new_text: String = self.text.iter().collect();
                            new_text = new_text.replace(self.selection_text().as_str(), content.unwrap().as_str());

                            self.set_text(new_text);
                        } else {
                            let mut new_text: String = self.text.iter().collect();
                            new_text.push_str(content.unwrap().as_str());

                            self.set_text(new_text);
                        }
                    }
                }

                if input.key_just_typed(VirtualKeyCode::X) {
                    self.clipboard_context.set_contents(self.selection_text()).unwrap();
                    self.set_text("".to_owned());
                }
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
