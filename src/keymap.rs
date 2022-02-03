//! Keyboard shortcut mappings.

use crate::backend::winit::event::VirtualKeyCode;
use serde::{Deserialize, Serialize};

use crate::ui::Modifier;

/// A key binding with a modifier.
pub type KeyBinding = (Modifier, VirtualKeyCode);

/// The key map.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Keymap {
   pub edit: EditKeymap,
   #[serde(default)]
   pub tools: ToolKeymap,
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

/// The key map for selecting tools.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ToolKeymap {
   pub selection: KeyBinding,
   pub brush: KeyBinding,
   pub eyedropper: KeyBinding,
}

impl Default for ToolKeymap {
   fn default() -> Self {
      Self {
         selection: (Modifier::NONE, VirtualKeyCode::Key1),
         brush: (Modifier::NONE, VirtualKeyCode::Key2),
         eyedropper: (Modifier::NONE, VirtualKeyCode::Key3),
      }
   }
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
         tools: Default::default(),
         brush: BrushKeymap {
            decrease_thickness: (Modifier::NONE, VirtualKeyCode::LBracket),
            increase_thickness: (Modifier::NONE, VirtualKeyCode::RBracket),
         },
      }
   }
}
