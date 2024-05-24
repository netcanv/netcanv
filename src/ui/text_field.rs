//! A fairly simplistic text field implementation.

use std::ops::Range;

use crate::backend::winit::keyboard::{Key, NamedKey};
use crate::backend::winit::window::CursorIcon;
use netcanv_renderer::Font as FontTrait;
use paws::{point, vector, AlignH, AlignV, Color, Layout, LineCap, Rect, Renderer};

use crate::backend::Font;
use crate::clipboard;
use crate::config::config;
use crate::ui::*;

/// A text field's state.
pub struct TextField {
   text: String,

   focused: bool,
   selection: Selection,

   blink_start: f32,
   scroll_x: f32,
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

         scroll_x: 0.0,
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
      input: &mut Input,
      TextFieldArgs {
         font,
         width,
         colors,
         hint,
      }: TextFieldArgs,
   ) -> TextFieldProcessResult {
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
      if ui.hover(input) {
         input.set_cursor(CursorIcon::Text);
      }

      ui.render().push();
      ui.clip();
      ui.render().translate(vector(-self.scroll_x, 0.0));

      // Rendering: hint
      if let Some(hint) = hint {
         if self.text.is_empty() {
            ui.text(font, hint, colors.text_hint, (AlignH::Left, AlignV::Middle));
         }
      }

      if self.focused
         && (input.time_in_seconds() - self.blink_start) % Self::BLINK_PERIOD < Self::HALF_BLINK
      {
         ui.draw(|ui| {
            let current_text = &self.text[..self.selection.cursor()];
            let x = font.text_width(current_text);

            // While we have the caret's horizontal position already calculated,
            // also process scrolling.
            self.scroll_x = x - ui.width() + 1.0;
            self.scroll_x = self.scroll_x.max(0.0);

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

      // Process events
      let process_result = self.process_events(ui, input, font);

      ui.pop();
      ui.pop();

      process_result
   }

   /// Returns the height of a labelled text field.
   pub fn labelled_height(font: &Font) -> f32 {
      16.0 + TextField::height(font)
   }

   /// Processes a text field with an extra label above it.
   pub fn with_label(
      &mut self,
      ui: &mut Ui,
      input: &mut Input,
      label_font: &Font,
      label: &str,
      args: TextFieldArgs,
   ) -> TextFieldProcessResult {
      ui.push(
         (args.width, Self::labelled_height(args.font)),
         Layout::Vertical,
      );

      // label
      ui.push((args.width, 16.0), Layout::Freeform);
      ui.text(
         label_font,
         label,
         args.colors.label,
         (AlignH::Left, AlignV::Top),
      );
      ui.pop();

      // field
      let process_result = self.process(ui, input, args);

      ui.pop();

      process_result
   }

   /// Returns the text in the text field.
   pub fn text(&self) -> &str {
      &self.text
   }

   /// Sets the text in the text field.
   pub fn set_text(&mut self, text: String) {
      self.text = text;
      self.selection.move_to(TextPosition(self.text.len()));
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
         self.selection.move_to(TextPosition(self.selection.start()));
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

   /// Returns the character index clicked based on an X position.
   fn get_text_position_from_x(&mut self, font: &Font, x: f32) -> TextPosition {
      let mut x_offset = 0.0;
      let mut last_index = 0;
      for (index, character) in self.text.char_indices() {
         let mut text = [0; 4];
         let text = character.encode_utf8(&mut text);
         let character_width = font.text_width(text);
         if x_offset >= x {
            return TextPosition(if x_offset - x > character_width / 2.0 {
               last_index
            } else {
               index
            });
         }
         x_offset += character_width;
         last_index = index;
      }
      TextPosition(self.text.len())
   }

   fn get_text_position_from_mouse(&mut self, ui: &Ui, input: &Input, font: &Font) -> TextPosition {
      let x = ui.mouse_position(input).x;
      self.get_text_position_from_x(font, x)
   }

   /// Processes input events.
   fn process_events(&mut self, ui: &Ui, input: &Input, font: &Font) -> TextFieldProcessResult {
      let mut process_result = TextFieldProcessResult {
         unfocused: false,
         done: false,
      };

      if input.action(MouseButton::Left) == (true, ButtonState::Pressed) {
         let was_focused = self.focused;
         self.focused = ui.hover(input);
         if self.focused {
            self.reset_blink(input);
            let position = self.get_text_position_from_mouse(ui, input, font);
            self.selection.move_to(position);
         }
         if !self.focused && was_focused {
            process_result.unfocused = true;
         }
      }
      if input.action(MouseButton::Left) == (true, ButtonState::Down) && self.focused {
         self.reset_blink(input);
         let position = self.get_text_position_from_mouse(ui, input, font);
         self.selection.cursor = position;
      }

      if self.focused {
         if !input.characters_typed().is_empty() {
            self.reset_blink(input);
         }

         // Most of these keybindings don't use the action system, as it would be quite cumbersome
         // and repetitive to represent all the possible textbox actions using it.

         if input.key_just_typed(Key::Named(NamedKey::ArrowLeft)) {
            self.reset_blink(input);

            if input.ctrl_is_down() {
               self.skip_word(ArrowKey::Left, input.shift_is_down());
            } else {
               self.selection.move_left(&self.text, input.shift_is_down());
            }
         }

         if input.key_just_typed(Key::Named(NamedKey::ArrowRight)) {
            self.reset_blink(input);

            if input.ctrl_is_down() {
               self.skip_word(ArrowKey::Right, input.shift_is_down());
            } else if self.selection.cursor() < self.text.len() {
               self.selection.move_right(&self.text, input.shift_is_down());
            }
         }

         if input.key_just_typed(Key::Named(NamedKey::Home)) {
            self.selection.move_to(TextPosition(0));
            self.reset_blink(input);
         }

         if input.key_just_typed(Key::Named(NamedKey::End)) {
            self.selection.move_to(TextPosition(self.text.len()));
            self.reset_blink(input);
         }

         if input.action(&config().keymap.edit.select_all) == (true, true) {
            self.selection.anchor = TextPosition(0);
            self.selection.cursor = TextPosition(self.text.len());
         }

         if input.action(&config().keymap.edit.copy) == (true, true) {
            catch!(
               clipboard::copy_string(self.selection_text().to_owned()),
               return process_result
            );
         }

         if input.action(&config().keymap.edit.paste) == (true, true) {
            if let Ok(clipboard) = clipboard::paste_string() {
               let clipboard = clipboard.replace('\n', " ");
               let start = self.selection.start();
               self.text.replace_range(self.selection.normalize(), &clipboard);
               self.selection.move_to(TextPosition(start + clipboard.len()));
            }
         }

         if input.action(&config().keymap.edit.cut) == (true, true) {
            catch!(
               clipboard::copy_string(self.selection_text().to_owned()),
               return process_result
            );
            self.backspace();
         }

         if input.key_just_typed(Key::Named(NamedKey::Backspace)) {
            if input.ctrl_is_down() {
               // Simulate the shift key being held down while moving to the word on the left, so as
               // to select the word to the left and then backspace it.
               self.skip_word(ArrowKey::Left, true);
            }
            self.backspace();
         }

         if input.key_just_typed(Key::Named(NamedKey::Delete)) {
            if input.ctrl_is_down() {
               // Similar to backspace, simulate the shift key being held down, but this time
               // while moving over to the right.
               self.skip_word(ArrowKey::Right, true);
            }
            self.delete();
         }

         if input.key_just_typed(Key::Named(NamedKey::Enter)) {
            process_result.done = true;
         }

         for ch in input.characters_typed() {
            if !ch.is_control() {
               self.append(*ch);
            }
         }
      } else {
         self.selection.anchor = self.selection.cursor;
      }

      process_result
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

/// The result of processing a text field.
pub struct TextFieldProcessResult {
   done: bool,
   unfocused: bool,
}

impl TextFieldProcessResult {
   /// Returns whether the user pressed the Return key while editing text.
   pub fn done(&self) -> bool {
      self.done
   }

   /// Returns whether the text field was unfocused.
   pub fn unfocused(&self) -> bool {
      self.unfocused
   }
}

/// A position inside of a string. This position can be moved left or right.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TextPosition(usize);

// This impl groups bits to make UTF-8 decoding more readable.
#[allow(clippy::unusual_byte_groupings)]
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
