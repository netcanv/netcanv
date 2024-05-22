//! Keyboard shortcut mappings.

use serde::{Deserialize, Serialize};
use crate::backend::winit::keyboard::{Key, NamedKey};

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
   pub copy: Key,
   pub cut: Key,
   pub paste: Key,
   pub delete: Key,
   pub select_all: Key,
}

/// The key map for selecting tools.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ToolKeymap {
   pub selection: Key,
   pub brush: Key,
   pub eyedropper: Key,
}

impl Default for ToolKeymap {
   fn default() -> Self {
      Self {
         selection: Key::Character("1".into()),
         brush: Key::Character("2".into()),
         eyedropper: Key::Character("3".into()),
      }
   }
}

/// The key mappings for the brush tool.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BrushKeymap {
   pub decrease_thickness: Key,
   pub increase_thickness: Key,
}

impl Default for Keymap {
   fn default() -> Self {
      Self {
         edit: EditKeymap {
            copy: Key::Character("C".into()),
            cut: Key::Character("X".into()),
            paste: Key::Character("V".into()),
            delete: Key::Named(NamedKey::Delete),
            select_all: Key::Character("A".into()),
         },
         tools: Default::default(),
         brush: BrushKeymap {
            decrease_thickness: Key::Character("[".into()),
            increase_thickness: Key::Character("]".into()),
         },
      }
   }
}
