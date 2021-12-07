//! Handling of assets such as icons, fonts, etc.

use netcanv_renderer::paws::Color;
use netcanv_renderer::RenderBackend;

use crate::backend::{Backend, Font, Image};
use crate::ui::wm::windows::{WindowButtonColors, WindowButtonsColors};
use crate::ui::{
   ButtonColors, ColorPickerIcons, ContextMenuColors, ExpandColors, ExpandIcons, RadioButtonColors,
   TextFieldColors,
};

const SANS_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");
const MONOSPACE_TTF: &[u8] = include_bytes!("assets/fonts/RobotoMono-Medium.ttf");

const CHEVRON_RIGHT_SVG: &[u8] = include_bytes!("assets/icons/chevron-right.svg");
const CHEVRON_DOWN_SVG: &[u8] = include_bytes!("assets/icons/chevron-down.svg");
const ERASER_SVG: &[u8] = include_bytes!("assets/icons/eraser.svg");
const MENU_SVG: &[u8] = include_bytes!("assets/icons/menu.svg");
const COPY_SVG: &[u8] = include_bytes!("assets/icons/copy.svg");
const INFO_SVG: &[u8] = include_bytes!("assets/icons/info.svg");
const ERROR_SVG: &[u8] = include_bytes!("assets/icons/error.svg");
const PEER_CLIENT_SVG: &[u8] = include_bytes!("assets/icons/peer-client.svg");
const PEER_HOST_SVG: &[u8] = include_bytes!("assets/icons/peer-host.svg");
const SAVE_SVG: &[u8] = include_bytes!("assets/icons/save.svg");
const DARK_MODE_SVG: &[u8] = include_bytes!("assets/icons/dark-mode.svg");
const LIGHT_MODE_SVG: &[u8] = include_bytes!("assets/icons/light-mode.svg");
const WINDOW_CLOSE_SVG: &[u8] = include_bytes!("assets/icons/window-close.svg");
const WINDOW_PIN_SVG: &[u8] = include_bytes!("assets/icons/window-pin.svg");
const WINDOW_PINNED_SVG: &[u8] = include_bytes!("assets/icons/window-pinned.svg");

/// Icons for navigation.
pub struct NavigationIcons {
   pub menu: Image,
   pub copy: Image,
}

/// Icons for status messages.
pub struct StatusIcons {
   pub info: Image,
   pub error: Image,
}

/// Icons for file operations.
pub struct FileIcons {
   pub save: Image,
}

/// Icons for peer roles.
pub struct PeerIcons {
   pub client: Image,
   pub host: Image,
}

/// Icons for the color scheme switcher.
pub struct ColorSwitcherIcons {
   pub dark: Image,
   pub light: Image,
}

pub struct WindowIcons {
   pub close: Image,
   pub pin: Image,
   pub pinned: Image,
}

/// Icons, rendered to images at startup.
pub struct Icons {
   // Control-specific
   pub expand: ExpandIcons,
   pub color_picker: ColorPickerIcons,
   pub color_switcher: ColorSwitcherIcons,

   // Generic
   pub navigation: NavigationIcons,
   pub status: StatusIcons,
   pub file: FileIcons,
   pub peer: PeerIcons,
   pub window: WindowIcons,
}

/// App assets. This constitutes fonts, color schemes, icons, and the like.
pub struct Assets {
   pub sans: Font,
   pub sans_bold: Font,
   pub monospace: Font,

   pub colors: ColorScheme,
   pub icons: Icons,
}

impl Assets {
   /// Loads an icon from an SVG file.
   pub fn load_icon(renderer: &mut Backend, data: &[u8]) -> Image {
      use usvg::{FitTo, NodeKind, Tree};

      let tree =
         Tree::from_data(data, &Default::default()).expect("error while loading the SVG file");
      let size = match *tree.root().borrow() {
         NodeKind::Svg(svg) => svg.size,
         _ => panic!("the root node of the SVG is not <svg/>"),
      };
      let mut pixmap = tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32).unwrap();
      resvg::render(&tree, FitTo::Original, pixmap.as_mut());

      renderer.create_image_from_rgba(size.width() as u32, size.height() as u32, pixmap.data())
   }

   /// Creates a new instance of Assets with the provided color scheme.
   pub fn new(renderer: &mut Backend, colors: ColorScheme) -> Self {
      Self {
         sans: renderer.create_font_from_memory(SANS_TTF, 14.0),
         sans_bold: renderer.create_font_from_memory(SANS_BOLD_TTF, 14.0),
         monospace: renderer.create_font_from_memory(MONOSPACE_TTF, 14.0),
         colors,
         icons: Icons {
            expand: ExpandIcons {
               expand: Self::load_icon(renderer, CHEVRON_RIGHT_SVG),
               shrink: Self::load_icon(renderer, CHEVRON_DOWN_SVG),
            },
            color_picker: ColorPickerIcons {
               eraser: Self::load_icon(renderer, ERASER_SVG),
            },
            color_switcher: ColorSwitcherIcons {
               dark: Self::load_icon(renderer, DARK_MODE_SVG),
               light: Self::load_icon(renderer, LIGHT_MODE_SVG),
            },

            navigation: NavigationIcons {
               menu: Self::load_icon(renderer, MENU_SVG),
               copy: Self::load_icon(renderer, COPY_SVG),
            },
            status: StatusIcons {
               info: Self::load_icon(renderer, INFO_SVG),
               error: Self::load_icon(renderer, ERROR_SVG),
            },
            file: FileIcons {
               save: Self::load_icon(renderer, SAVE_SVG),
            },
            peer: PeerIcons {
               client: Self::load_icon(renderer, PEER_CLIENT_SVG),
               host: Self::load_icon(renderer, PEER_HOST_SVG),
            },
            window: WindowIcons {
               close: Self::load_icon(renderer, WINDOW_CLOSE_SVG),
               pin: Self::load_icon(renderer, WINDOW_PIN_SVG),
               pinned: Self::load_icon(renderer, WINDOW_PINNED_SVG),
            },
         },
      }
   }
}

/// A "rough overview" of a color scheme. Contains only the essential colors, and forms the basis
/// for a precise [`ColorScheme`].
struct CommonColors {
   gray_00: Color,
   gray_20: Color,
   gray_50: Color,
   gray_60: Color,
   gray_80: Color,
   gray_90: Color,

   red_10: Color,
   red_30: Color,

   blue_30: Color,
   blue_50: Color,
   blue_70: Color,

   white: Color,
}

impl CommonColors {
   /// The common colors for the light theme.
   fn light() -> Self {
      Self {
         gray_00: Color::BLACK,
         gray_20: Color::rgb(0x333333),
         gray_50: Color::rgb(0x7f7f7f),
         gray_60: Color::rgb(0xa9a9a9),
         gray_80: Color::rgb(0xeeeeee),
         gray_90: Color::WHITE,

         red_10: Color::rgb(0x3d0011),
         red_30: Color::rgb(0x7d0023),

         blue_30: Color::rgb(0x007ccf),
         blue_50: Color::rgb(0x0397fb),
         blue_70: Color::rgb(0x32aafa),

         white: Color::WHITE,
      }
   }

   /// The common colors for the dark theme.
   fn dark() -> Self {
      Self {
         gray_00: Color::rgb(0xb7b7b7),
         gray_20: Color::rgb(0xa0a0a0),
         gray_50: Color::rgb(0x6f6f6f),
         gray_60: Color::rgb(0x343434),
         gray_80: Color::rgb(0x1f1f1f),
         gray_90: Color::rgb(0x383838),

         red_10: Color::rgb(0xdb325a),
         red_30: Color::rgb(0xff7593),

         blue_30: Color::rgb(0x007ccf),
         blue_50: Color::rgb(0x0397fb),
         blue_70: Color::rgb(0x32aafa),

         white: Color::WHITE,
      }
   }
}

/// A color scheme.
#[derive(Clone)]
pub struct ColorScheme {
   pub text: Color,
   pub panel: Color,
   pub separator: Color,
   pub error: Color,

   pub button: ButtonColors,
   pub action_button: ButtonColors,
   pub toolbar_button: ButtonColors,
   pub selected_toolbar_button: ButtonColors,
   pub radio_button: RadioButtonColors,
   pub expand: ExpandColors,
   pub slider: Color,
   pub text_field: TextFieldColors,
   pub context_menu: ContextMenuColors,
   pub window_buttons: WindowButtonsColors,

   pub titlebar: TitlebarColors,
}

impl ColorScheme {
   /// Constructs and returns the light color scheme.
   pub fn light() -> Self {
      Self::from(CommonColors::light())
   }

   /// Constructs and returns the dark color scheme.
   pub fn dark() -> Self {
      Self::from(CommonColors::dark())
   }
}

impl From<CommonColors> for ColorScheme {
   fn from(
      CommonColors {
         gray_00,
         gray_20,
         gray_50,
         gray_60,
         gray_80,
         gray_90,
         red_10,
         red_30,
         blue_30,
         blue_50,
         blue_70,
         white,
      }: CommonColors,
   ) -> Self {
      let black_hover = gray_00.with_alpha(48);
      let black_pressed = gray_00.with_alpha(96);
      let white_hover = gray_90.with_alpha(48);
      let white_pressed = gray_90.with_alpha(16);

      let separator = gray_60;

      Self {
         text: gray_00,
         panel: gray_80,
         separator,
         error: red_30,

         button: ButtonColors {
            fill: Color::TRANSPARENT,
            outline: gray_50,
            text: gray_00,
            hover: black_hover,
            pressed: black_pressed,
         },
         action_button: ButtonColors {
            fill: Color::TRANSPARENT,
            outline: Color::TRANSPARENT,
            text: gray_00,
            hover: black_hover,
            pressed: black_pressed,
         },
         toolbar_button: ButtonColors {
            fill: Color::TRANSPARENT,
            outline: Color::TRANSPARENT,
            text: gray_20,
            hover: black_hover,
            pressed: black_pressed,
         },
         selected_toolbar_button: ButtonColors {
            fill: gray_20,
            outline: Color::TRANSPARENT,
            text: gray_80,
            hover: white_hover,
            pressed: white_pressed,
         },
         radio_button: RadioButtonColors {
            normal: ButtonColors {
               fill: Color::TRANSPARENT,
               outline: gray_50,
               text: gray_00,
               hover: black_hover,
               pressed: black_pressed,
            },
            selected: ButtonColors {
               fill: gray_20,
               outline: Color::TRANSPARENT,
               text: gray_80,
               hover: white_hover,
               pressed: white_pressed,
            },
         },
         slider: gray_00,
         expand: ExpandColors {
            icon: gray_00,
            text: gray_00,
            hover: black_hover,
            pressed: black_pressed,
         },
         text_field: TextFieldColors {
            outline: gray_50,
            outline_focus: gray_20,
            fill: gray_90,
            text: gray_00,
            text_hint: gray_50,
            label: gray_00,
            selection: blue_70,
         },
         context_menu: ContextMenuColors {
            background: gray_80,
         },
         window_buttons: WindowButtonsColors {
            close: WindowButtonColors {
               normal_fill: Color::TRANSPARENT,
               normal_icon: gray_00,
               hover_fill: red_30,
               hover_icon: gray_90,
               pressed_fill: red_10,
               pressed_icon: gray_80,
            },
            pin: WindowButtonColors {
               normal_fill: Color::TRANSPARENT,
               normal_icon: gray_00,
               hover_fill: black_hover,
               hover_icon: gray_00,
               pressed_fill: black_pressed,
               pressed_icon: gray_00,
            },
            pinned: WindowButtonColors {
               normal_fill: blue_50,
               normal_icon: white,
               hover_fill: blue_70,
               hover_icon: white,
               pressed_fill: blue_30,
               pressed_icon: white,
            },
         },

         titlebar: TitlebarColors {
            titlebar: gray_90,
            separator,
            text: gray_00,

            foreground_hover: gray_80,
            button: gray_00,
            close_button: red_30,
         },
      }
   }
}

impl From<crate::config::ColorScheme> for ColorScheme {
   fn from(scheme: crate::config::ColorScheme) -> Self {
      use crate::config::ColorScheme;
      match scheme {
         ColorScheme::Light => Self::light(),
         ColorScheme::Dark => Self::dark(),
      }
   }
}

/// The title bar's color scheme. This only applies to title bars on Wayland, where the compositor
/// does not always provide a server-side title bar.
#[derive(Clone)]
pub struct TitlebarColors {
   pub titlebar: Color,
   pub separator: Color,
   pub text: Color,

   pub foreground_hover: Color,
   pub button: Color,
   pub close_button: Color,
}
