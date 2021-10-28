//! The trait all app states must implement.

use crate::ui::{input::Input, Ui};

/// Arguments passed to app states.
pub struct StateArgs<'a, 'b> {
   pub ui: &'a mut Ui,
   pub input: &'b mut Input,
}

/// Trait implemented by all app states.
pub trait AppState {
   /// Processes a single frame.
   ///
   /// In NetCanv, input handling and drawing are done at the same time, which is called
   /// _processing_ in the codebase.
   fn process(&mut self, args: StateArgs);

   /// Returns the next state after this one.
   ///
   /// If no state transitions should occur, this should simply return `self`. Otherwise, another
   /// app state may be constructed, boxed, and returned.
   fn next_state(self: Box<Self>) -> Box<dyn AppState>;
}
