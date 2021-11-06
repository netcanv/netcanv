//! A fairly simplistic text field implementation.

use std::ops::Range;

use copypasta::{ClipboardContext, ClipboardProvider};
use netcanv_renderer::Font as FontTrait;
use paws::{point, vector, AlignH, AlignV, Color, Layout, LineCap, Rect, Renderer};

use crate::{backend::Font, ui::*};

/// A text field's state.
pub struct TextField {
   text: String,
   focused: bool,
   blink_start: f32,

   selection: Selection,

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
pub struct TextFieldArgs<'a, 'b, 'c> {
   pub width: f32,
   pub colors: &'a TextFieldColors,
   pub hint: Option<&'b str>,
   pub font: &'c Font,
}

impl TextField {
   /// The blinking period of the caret.
   const BLINK_PERIOD: f32 = 1.0;
   const HALF_BLINK: f32 = Self::BLINK_PERIOD / 2.0;

   /// Creates a new text field, with the optionally provided initial text.
   pub fn new(initial_text: Option<&str>) -> Self {
      let text = initial_text.unwrap_or("").to_owned();
      let length = text.len();

      Self {
         text,
         focused: false,
         blink_start: 0.0,

         selection: Selection {
            cursor: TextPosition(length),
            anchor: TextPosition(length),
         },

         clipboard_context: ClipboardContext::new().unwrap(),
      }
   }

   /// Returns the height of a text field.
   pub fn height(font: &Font) -> f32 {
      f32::round(16.0 / 7.0 * font.size())
   }

   /// Processes a text field.
   pub fn process(
      &mut self,
      ui: &mut Ui,
      input: &Input,
      TextFieldArgs {
         font,
         width,
         colors,
         hint,
      }: TextFieldArgs,
   ) {
      ui.push((width, Self::height(font)), Layout::Freeform);

      // Rendering: box
      let outline_color = if self.focused {
         colors.outline_focus
      } else {
         colors.outline
      };
      ui.fill_rounded(colors.fill, 4.0);
      ui.outline_rounded(outline_color, 4.0, 1.0);

      ui.push(ui.size(), Layout::Freeform);
      ui.pad((8.0, 0.0));

      ui.render().push();
      ui.clip();

      // Rendering: hint
      if hint.is_some() && self.text.len() == 0 {
         ui.text(
            font,
            hint.unwrap(),
            colors.text_hint,
            (AlignH::Left, AlignV::Middle),
         );
      }

      if !self.focused {
         self.selection.anchor = self.selection.cursor;
      }

      if self.focused
         && (input.time_in_seconds() - self.blink_start) % Self::BLINK_PERIOD < Self::HALF_BLINK
      {
         ui.draw(|ui| {
            let current_text = &self.text[..self.selection.cursor()];
            let current_text_width = font.text_width(&current_text);

            let x = current_text_width;
            let y1 = (Self::height(font) * 0.2).round();
            let y2 = (Self::height(font) * 0.8).round();
            ui.line(point(x, y1), point(x, y2), colors.text, LineCap::Butt, 1.0);
         });
      }

      if self.selection.cursor != self.selection.anchor {
         ui.draw(|ui| {
            // Get all the text starting from the start of the textbox to the first position
            // of the selection.
            // From this, we can calculate where to position the selection rectangle.
            let selection_start = &self.text[..self.selection.start()];
            let selection_x = font.text_width(selection_start).round();

            // Get all the selected text and its width.
            let selected_text = &self.text[self.selection.normalize()];
            let selection_width = font.text_width(selected_text).round();

            let y = (Self::height(font) * 0.2).round();
            let height = (Self::height(font) * 0.6).ceil();

            ui.render().fill(
               Rect::new(point(selection_x, y), vector(selection_width, height)),
               colors.selection,
               0.0,
            )
         });
      }

      ui.text(
         font,
         &self.text,
         colors.text,
         (AlignH::Left, AlignV::Middle),
      );

      ui.render().pop();
      ui.pop();

      // Process events
      self.process_events(ui, input);

      ui.pop();
   }

   /// Returns the selection contents.
   fn selection_text(&self) -> &str {
      &self.text[self.selection.normalize()]
   }

   /// Resets the text field's blink timer.
   fn reset_blink(&mut self, input: &Input) {
      self.blink_start = input.time_in_seconds();
   }

   /// Appends a character to the cursor position, or replaces selection if any.
   fn append(&mut self, ch: char) {
      if self.selection.len() > 0 {
         let mut bytes = [0; 4];
         self.text.replace_range(self.selection.normalize(), ch.encode_utf8(&mut bytes));
         self.selection.move_right(&self.text, false);
      } else {
         self.text.insert(self.selection.cursor(), ch);
         self.selection.move_right(&self.text, false);
      }
   }

   /// Removes a character at cursor position, or removes selection if any.
   fn backspace(&mut self) {
      if self.selection.len() != 0 {
         self.delete();
      } else if self.selection.cursor() > 0 {
         self.selection.move_left(&self.text, false);
         self.text.remove(self.selection.cursor());
      }
   }

   /// Removes character after cursor position.
   /// Or selection if any.
   fn delete(&mut self) {
      if self.selection.len() != 0 {
         self.text.drain(self.selection.normalize());
         self.selection.move_to(TextPosition(self.selection.start()));
      } else if self.selection.cursor() != self.text.len() {
         self.text.remove(self.selection.cursor());
      }
   }

   /// Returns whether the Ctrl key is being held.
   fn ctrl_is_down(&self, input: &Input) -> bool {
      input.key_is_down(VirtualKeyCode::LControl) || input.key_is_down(VirtualKeyCode::RControl)
   }

   /// Returns whether the Shift key is being held.
   fn shift_is_down(&self, input: &Input) -> bool {
      input.key_is_down(VirtualKeyCode::LShift) || input.key_is_down(VirtualKeyCode::RShift)
   }

   /// Moves the cursor to the left as long as the given condition is satisfied.
   ///
   /// The condition receives the current character under the cursor on each iteration.
   fn skip_left(&mut self, condition: impl Fn(char) -> bool, is_shift_down: bool) {
      if self.selection.cursor() == 0 {
         return;
      }
      while let Some(c) = self.text.get_char(self.selection.cursor() - 1) {
         if condition(c) {
            self.selection.move_left(&self.text, is_shift_down);
            if self.selection.cursor() == 0 {
               break;
            }
         } else {
            break;
         }
      }
   }

   /// Moves the cursor to the right as long as the given condition is satisfied.
   ///
   /// The condition receives the current character under the cursor on each iteration.
   fn skip_right(&mut self, condition: impl Fn(char) -> bool, is_shift_down: bool) {
      if self.selection.cursor() >= self.text.len() {
         return;
      }
      while let Some(c) = self.text.get_char(self.selection.cursor()) {
         if condition(c) {
            self.selection.move_right(&self.text, is_shift_down);
            if self.selection.cursor() >= self.text.len() {
               break;
            }
         } else {
            break;
         }
      }
   }

   /// Skips a word left or right.
   fn skip_word(&mut self, arrow_key: ArrowKey, is_shift_down: bool) {
      match arrow_key {
         ArrowKey::Left => {
            self.skip_left(|c| c.is_whitespace(), is_shift_down);
            self.skip_left(|c| !c.is_whitespace(), is_shift_down);
         }
         ArrowKey::Right => {
            self.skip_right(|c| c.is_whitespace(), is_shift_down);
            self.skip_right(|c| !c.is_whitespace(), is_shift_down);
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
            self.reset_blink(input);

            if self.ctrl_is_down(input) {
               self.skip_word(ArrowKey::Left, self.shift_is_down(input));
            } else {
               self.selection.move_left(&self.text, self.shift_is_down(input));
            }
         }

         if input.key_just_typed(VirtualKeyCode::Right) {
            self.reset_blink(input);

            if self.ctrl_is_down(input) {
               self.skip_word(ArrowKey::Right, self.shift_is_down(input));
            } else if self.selection.cursor() < self.text.len() {
               self.selection.move_right(&self.text, self.shift_is_down(input));
            }
         }

         if input.key_just_typed(VirtualKeyCode::Home) {
            self.selection.move_to(TextPosition(0));
            self.reset_blink(input);
         }

         if input.key_just_typed(VirtualKeyCode::End) {
            self.selection.move_to(TextPosition(self.text.len()));
            self.reset_blink(input);
         }

         if self.ctrl_is_down(input) && input.key_just_typed(VirtualKeyCode::A) {
            self.selection.anchor = TextPosition(0);
            self.selection.cursor = TextPosition(self.text.len());
         }

         if self.ctrl_is_down(input) && input.key_just_typed(VirtualKeyCode::C) {
            self.clipboard_context.set_contents(self.selection_text().to_owned()).unwrap();
         }

         if self.ctrl_is_down(input) && input.key_just_typed(VirtualKeyCode::V) {
            if let Ok(clipboard) = self.clipboard_context.get_contents() {
               let cursor = self.selection.cursor();
               self.text.replace_range(self.selection.normalize(), &clipboard);
               self.selection.move_to(TextPosition(cursor + clipboard.len()));
            }
         }

         if self.ctrl_is_down(input) && input.key_just_typed(VirtualKeyCode::X) {
            let _ = self.clipboard_context.set_contents(self.selection_text().to_owned());
            self.backspace();
         }

         if input.key_just_typed(VirtualKeyCode::Back) {
            if self.ctrl_is_down(input) {
               // Simulate the shift key being held down while moving to the word on the left, so as
               // to select the word to the left and then backspace it.
               self.skip_word(ArrowKey::Left, true);
            }
            self.backspace();
         }

         if input.key_just_typed(VirtualKeyCode::Delete) {
            if self.ctrl_is_down(input) {
               // Similar to backspace, simulate the shift key being held down, but this time
               // while moving over to the right.
               self.skip_word(ArrowKey::Right, true);
            }
            self.delete();
         }

         for ch in input.characters_typed() {
            if !ch.is_control() {
               self.append(*ch);
            }
         }
      }
   }

   /// Returns the height of a labelled text field.
   pub fn labelled_height(font: &Font) -> f32 {
      16.0 + TextField::height(font)
   }

   /// Processes a text field with an extra label above it.
   pub fn with_label(&mut self, ui: &mut Ui, input: &Input, label: &str, args: TextFieldArgs) {
      ui.push(
         (args.width, Self::labelled_height(args.font)),
         Layout::Vertical,
      );

      // label
      ui.push((args.width, 16.0), Layout::Freeform);
      ui.text(
         args.font,
         label,
         args.colors.label,
         (AlignH::Left, AlignV::Top),
      );
      ui.pop();

      // field
      self.process(ui, input, args);

      ui.pop();
   }

   /// Returns the text in the text field.
   pub fn text(&self) -> &str {
      &self.text
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

/// A position inside of a string. This position can be moved left or right.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition(usize);

impl TextPosition {
   /// Returns the next character position in the UTF-8 string.
   fn next(mut self, text: &str) -> Self {
      let bytes = text.as_bytes();
      if let Some(&byte) = bytes.get(self.0) {
         self.0 += match byte {
            0x00..=0x7f => 1,
            x if x & 0b111_00000 == 0b110_00000 => 2,
            x if x & 0b1111_0000 == 0b1110_0000 => 3,
            x if x & 0b11111_000 == 0b11110_000 => 4,
            _ => 1,
         };
      }
      self
   }

   /// Returns the previous character position in the UTF-8 string.
   fn previous(mut self, text: &str) -> Self {
      if self.0 == 0 {
         return self;
      }
      let bytes = text.as_bytes();
      while let Some(&byte) = bytes.get(self.0 - 1) {
         match byte {
            x if x & 0b11_000000 == 0b10_000000 => self.0 -= 1,
            _ => {
               self.0 -= 1;
               break;
            }
         }
      }
      self
   }
}

trait GetChar {
   fn get_char(&self, position: usize) -> Option<char>;
}

impl GetChar for String {
   fn get_char(&self, position: usize) -> Option<char> {
      self[position..].chars().next()
   }
}

/// Text field selection.
/// Stores two cursors: the text cursor and the selection anchor.
/// These cursors are modified appropriately as the user edits text.
struct Selection {
   /// The text cursor. This is the actual position of the caret.
   cursor: TextPosition,
   /// The selection anchor. This is the position at which the selection was started.
   anchor: TextPosition,
}

impl Selection {
   /// Returns the selection's cursor position, as a `usize`.
   pub fn cursor(&self) -> usize {
      self.cursor.0
   }

   /// Returns the first (earlier) position in the selection.
   pub fn start(&self) -> usize {
      self.cursor.0.min(self.anchor.0)
   }

   /// Returns the second (later) position in the selection.
   pub fn end(&self) -> usize {
      self.cursor.0.max(self.anchor.0)
   }

   /// Returns the selection, normalized. Equivalent to `self.start()..self.end()`.
   pub fn normalize(&self) -> Range<usize> {
      self.start()..self.end()
   }

   /// Returns the length of the selection.
   pub fn len(&self) -> usize {
      self.end() - self.start()
   }

   /// Moves the selection to the given position.
   pub fn move_to(&mut self, position: TextPosition) {
      self.cursor = position;
      self.anchor = self.cursor;
   }

   /// Moves the cursor left.
   ///
   /// If `is_shift_down` is true, only moves the cursor, but not the selection anchor.
   pub fn move_left(&mut self, text: &str, is_shift_down: bool) {
      if !is_shift_down && self.len() > 0 {
         self.cursor = TextPosition(self.start());
      } else {
         self.cursor = self.cursor.previous(text);
      }
      if !is_shift_down {
         self.anchor = self.cursor;
      }
   }

   /// Moves the cursor right.
   ///
   /// If `is_shift_down` is true, only moves the cursor, but not the selection anchor.
   pub fn move_right(&mut self, text: &str, is_shift_down: bool) {
      if !is_shift_down && self.len() > 0 {
         self.cursor = TextPosition(self.end());
      } else {
         self.cursor = self.cursor.next(text);
      }
      if !is_shift_down {
         self.anchor = self.cursor;
      }
   }
}

/// An arrow key.
enum ArrowKey {
   Left,
   Right,
}
