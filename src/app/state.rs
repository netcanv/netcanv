//! The trait all app states must implement.

use crate::backend::Backend;
use crate::ui::view::View;
use crate::ui::{Input, Ui};

/// Arguments passed to app states.
#[non_exhaustive]
pub struct StateArgs<'a, 'b> {
   pub ui: &'a mut Ui,
   pub input: &'b mut Input,
   pub root_view: View,
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
   fn next_state(self: Box<Self>, renderer: &mut Backend) -> Box<dyn AppState>;
}
