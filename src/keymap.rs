//! Keyboard shortcut mappings.

use crate::backend::winit::keyboard::{Key, NamedKey};
use serde::{Deserialize, Serialize};

use crate::ui::Modifier;

/// A key binding with a modifier.
pub type KeyBinding = (Modifier, Key);

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
         selection: (Modifier::NONE, Key::Character("1".into())),
         brush: (Modifier::NONE, Key::Character("2".into())),
         eyedropper: (Modifier::NONE, Key::Character("3".into())),
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
            copy: (Modifier::CTRL, Key::Character("c".into())),
            cut: (Modifier::CTRL, Key::Character("x".into())),
            paste: (Modifier::CTRL, Key::Character("v".into())),
            delete: (Modifier::NONE, Key::Named(NamedKey::Delete)),
            select_all: (Modifier::CTRL, Key::Character("a".into())),
         },
         tools: Default::default(),
         brush: BrushKeymap {
            decrease_thickness: (Modifier::NONE, Key::Character("[".into())),
            increase_thickness: (Modifier::NONE, Key::Character("]".into())),
         },
      }
   }
}
