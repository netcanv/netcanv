//! Keyboard shortcut mappings.

use netcanv_renderer_opengl::winit::event::VirtualKeyCode;
use serde::{Deserialize, Serialize};

use crate::ui::Modifier;

/// A key binding with a modifier.
type KeyBinding = (Modifier, VirtualKeyCode);

/// The key map.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Keymap {
   pub edit: EditKeymap,
   pub brush: BrushKeymap,
}

/// The key map for common editing actions, such as copying and pasting.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct EditKeymap {
   pub copy: KeyBinding,
   pub cut: KeyBinding,
   pub paste: KeyBinding,
   pub delete: KeyBinding,
   pub select_all: KeyBinding,
}

/// The key mappings for the brush tool.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BrushKeymap {
   pub decrease_thickness: KeyBinding,
   pub increase_thickness: KeyBinding,
}

impl Default for Keymap {
   fn default() -> Self {
      Self {
         edit: EditKeymap {
            copy: (Modifier::CTRL, VirtualKeyCode::C),
            cut: (Modifier::CTRL, VirtualKeyCode::X),
            paste: (Modifier::CTRL, VirtualKeyCode::V),
            delete: (Modifier::NONE, VirtualKeyCode::Delete),
            select_all: (Modifier::CTRL, VirtualKeyCode::A),
         },
         brush: BrushKeymap {
            decrease_thickness: (Modifier::NONE, VirtualKeyCode::LBracket),
            increase_thickness: (Modifier::NONE, VirtualKeyCode::RBracket),
         },
      }
   }
}
