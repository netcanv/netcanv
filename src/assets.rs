//! Handling of assets such as icons, fonts, etc.

use netcanv_renderer::paws::Color;
use netcanv_renderer::{Font as FontTrait, Image as ImageTrait};

use crate::backend::{Font, Image};
use crate::ui::{ButtonColors, ExpandColors, ExpandIcons, TextFieldColors};

const SANS_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

const CHEVRON_RIGHT_SVG: &[u8] = include_bytes!("assets/icons/chevron-right.svg");
const CHEVRON_DOWN_SVG: &[u8] = include_bytes!("assets/icons/chevron-down.svg");
const INFO_SVG: &[u8] = include_bytes!("assets/icons/info.svg");
const ERROR_SVG: &[u8] = include_bytes!("assets/icons/error.svg");
const SAVE_SVG: &[u8] = include_bytes!("assets/icons/save.svg");
const DARK_MODE_SVG: &[u8] = include_bytes!("assets/icons/dark-mode.svg");
const LIGHT_MODE_SVG: &[u8] = include_bytes!("assets/icons/light-mode.svg");

/// A color scheme.
#[derive(Clone)]
pub struct ColorScheme {
   pub text: Color,
   pub panel: Color,
   pub panel2: Color,
   pub separator: Color,
   pub error: Color,

   pub button: ButtonColors,
   pub action_button: ButtonColors,
   pub toolbar_button: ButtonColors,
   pub selected_toolbar_button: ButtonColors,
   pub expand: ExpandColors,
   pub slider: Color,
   pub text_field: TextFieldColors,

   pub titlebar: TitlebarColors,
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

/// Icons for the color scheme switcher.
pub struct ColorSwitcherIcons {
   pub dark: Image,
   pub light: Image,
}

/// Icons, rendered to images at startup.
pub struct Icons {
   pub expand: ExpandIcons,
   pub status: StatusIcons,
   pub file: FileIcons,
   pub color_switcher: ColorSwitcherIcons,
}

/// App assets. This constitutes fonts, color schemes, icons, and the like.
pub struct Assets {
   pub sans: Font,
   pub sans_bold: Font,

   pub colors: ColorScheme,
   pub icons: Icons,
}

impl Assets {
   /// Loads an icon from an SVG file.
   pub fn load_icon(data: &[u8]) -> Image {
      use usvg::{FitTo, NodeKind, Tree};

      let tree =
         Tree::from_data(data, &Default::default()).expect("error while loading the SVG file");
      let size = match *tree.root().borrow() {
         NodeKind::Svg(svg) => svg.size,
         _ => panic!("the root node of the SVG is not <svg/>"),
      };
      let mut pixmap = tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32).unwrap();
      resvg::render(&tree, FitTo::Original, pixmap.as_mut());

      Image::from_rgba(size.width() as u32, size.height() as u32, pixmap.data())
   }

   /// Creates a new instance of Assets with the provided color scheme.
   pub fn new(colors: ColorScheme) -> Self {
      Self {
         sans: Font::from_memory(SANS_TTF, 14.0),
         sans_bold: Font::from_memory(SANS_BOLD_TTF, 14.0),
         colors,
         icons: Icons {
            expand: ExpandIcons {
               expand: Self::load_icon(CHEVRON_RIGHT_SVG),
               shrink: Self::load_icon(CHEVRON_DOWN_SVG),
            },
            status: StatusIcons {
               info: Self::load_icon(INFO_SVG),
               error: Self::load_icon(ERROR_SVG),
            },
            file: FileIcons {
               save: Self::load_icon(SAVE_SVG),
            },
            color_switcher: ColorSwitcherIcons {
               dark: Self::load_icon(DARK_MODE_SVG),
               light: Self::load_icon(LIGHT_MODE_SVG),
            },
         },
      }
   }
}

impl ColorScheme {
   /// Constructs and returns the light color scheme.
   pub fn light() -> Self {
      Self {
         text: Color::argb(0xff000000),
         panel: Color::argb(0xffeeeeee),
         panel2: Color::argb(0xffffffff),
         separator: Color::argb(0xff202020),
         error: Color::argb(0xff7f0000),

         button: ButtonColors {
            fill: Color::argb(0x00000000),
            outline: Color::argb(0x60000000),
            text: Color::argb(0xff000000),
            hover: Color::argb(0x40000000),
            pressed: Color::argb(0x70000000),
         },
         action_button: ButtonColors {
            fill: Color::argb(0x00000000),
            outline: Color::argb(0x00000000),
            text: Color::argb(0xff000000),
            hover: Color::argb(0x40000000),
            pressed: Color::argb(0x70000000),
         },
         toolbar_button: ButtonColors {
            fill: Color::argb(0x00000000),
            outline: Color::argb(0x00000000),
            text: Color::argb(0xff333333),
            hover: Color::argb(0x40000000),
            pressed: Color::argb(0x70000000),
         },
         selected_toolbar_button: ButtonColors {
            fill: Color::argb(0xff333333),
            outline: Color::argb(0x00000000),
            text: Color::argb(0xffeeeeee),
            hover: Color::argb(0x40ffffff),
            pressed: Color::argb(0x70000000),
         },
         slider: Color::argb(0xff000000),
         expand: ExpandColors {
            icon: Color::argb(0xff000000),
            text: Color::argb(0xff000000),
            hover: Color::argb(0x40000000),
            pressed: Color::argb(0x70000000),
         },
         text_field: TextFieldColors {
            outline: Color::argb(0xff808080),
            outline_focus: Color::argb(0xff303030),
            fill: Color::argb(0xffffffff),
            text: Color::argb(0xff000000),
            text_hint: Color::argb(0x7f000000),
            label: Color::argb(0xff000000),
            selection: Color::argb(0x33000000),
         },
         titlebar: TitlebarColors {
            titlebar: Color::argb(0xffffffff),
            separator: Color::argb(0x7f000000),
            text: Color::argb(0xff000000),

            foreground_hover: Color::argb(0xffeeeeee),
            button: Color::argb(0xff000000),
         },
      }
   }

   /// Constructs and returns the dark color scheme.
   pub fn dark() -> Self {
      Self {
         text: Color::argb(0xffb7b7b7),
         panel: Color::argb(0xff1f1f1f),
         panel2: Color::argb(0xffffffff),
         separator: Color::argb(0xff202020),
         error: Color::argb(0xfffc9292),

         button: ButtonColors {
            fill: Color::argb(0x00000000),
            outline: Color::argb(0xff444444),
            text: Color::argb(0xffd2d2d2),
            hover: Color::argb(0x10ffffff),
            pressed: Color::argb(0x05ffffff),
         },
         action_button: ButtonColors {
            fill: Color::argb(0x00000000),
            outline: Color::argb(0x00000000),
            text: Color::argb(0xffb7b7b7),
            hover: Color::argb(0x20ffffff),
            pressed: Color::argb(0x05ffffff),
         },
         toolbar_button: ButtonColors {
            fill: Color::argb(0x00000000),
            outline: Color::argb(0x00000000),
            text: Color::argb(0xffb7b7b7),
            hover: Color::argb(0x20ffffff),
            pressed: Color::argb(0x05ffffff),
         },
         selected_toolbar_button: ButtonColors {
            fill: Color::argb(0xffa0a0a0),
            outline: Color::argb(0x00000000),
            text: Color::argb(0xff1f1f1f),
            hover: Color::argb(0x20ffffff),
            pressed: Color::argb(0x05ffffff),
         },
         slider: Color::argb(0xff979797),
         expand: ExpandColors {
            icon: Color::argb(0xffb7b7b7),
            text: Color::argb(0xffb7b7b7),
            hover: Color::argb(0x30ffffff),
            pressed: Color::argb(0x15ffffff),
         },
         text_field: TextFieldColors {
            outline: Color::argb(0xff595959),
            outline_focus: Color::argb(0xff9a9a9a),
            fill: Color::argb(0xff383838),
            text: Color::argb(0xffd5d5d5),
            text_hint: Color::argb(0x7f939393),
            label: Color::argb(0xffd5d5d5),
            selection: Color::argb(0x7f939393),
         },
         titlebar: TitlebarColors {
            titlebar: Color::argb(0xff383838),
            separator: Color::argb(0x7f939393),
            text: Color::argb(0xffd5d5d5),

            foreground_hover: Color::argb(0xff1f1f1f),
            button: Color::argb(0xffb7b7b7),
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
}

#[cfg(target_family = "unix")]
use crate::backend::winit::platform::unix::*;

#[cfg(target_family = "unix")]
fn winit_argb_from_skia_color(color: Color) -> ARGBColor {
   ARGBColor {
      a: color.a,
      r: color.r,
      g: color.g,
      b: color.b,
   }
}

#[cfg(target_family = "unix")]
impl Theme for ColorScheme {
   fn element_color(&self, element: Element, _window_active: bool) -> ARGBColor {
      match element {
         Element::Bar => winit_argb_from_skia_color(self.titlebar.titlebar),
         Element::Separator => winit_argb_from_skia_color(self.titlebar.separator),
         Element::Text => winit_argb_from_skia_color(self.titlebar.text),
      }
   }

   fn button_color(
      &self,
      button: Button,
      state: ButtonState,
      foreground: bool,
      _window_active: bool,
   ) -> ARGBColor {
      let color = match button {
         Button::Close => winit_argb_from_skia_color(self.error),
         Button::Maximize => winit_argb_from_skia_color(self.titlebar.button),
         Button::Minimize => winit_argb_from_skia_color(self.titlebar.button),
      };

      if foreground {
         if state == ButtonState::Hovered {
            return winit_argb_from_skia_color(self.titlebar.foreground_hover);
         } else {
            return winit_argb_from_skia_color(self.titlebar.text);
         }
      }

      match state {
         ButtonState::Disabled => winit_argb_from_skia_color(self.titlebar.separator),
         ButtonState::Hovered => color,
         ButtonState::Idle => winit_argb_from_skia_color(self.titlebar.titlebar),
      }
   }
}

/// A bus message notifying the main event loop that the color scheme has been switched.
/// Relevant only on Wayland, where the title bar is drawn by the application.
pub struct SwitchColorScheme(pub crate::config::ColorScheme);
