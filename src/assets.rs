//! Handling of assets such as icons, fonts, etc.

use std::io::{Cursor, Write};
use std::ops::Deref;

use netcanv_i18n::from_language::FromLanguage;
use netcanv_i18n::Language;
use netcanv_renderer::paws::Color;
use netcanv_renderer::{Image as ImageTrait, RenderBackend};
use serde::de::Visitor;
use serde::Deserialize;
use url::Url;

use crate::app::lobby::LobbyColors;
use crate::app::paint::tool_bar::ToolbarColors;
use crate::backend::{Backend, Font, Image};
use crate::config::config;
use crate::strings::Strings;
use crate::ui::wm::windows::{WindowButtonColors, WindowButtonsColors};
use crate::ui::{
   ButtonColors, ColorPickerIcons, ContextMenuColors, ExpandColors, ExpandIcons, RadioButtonColors,
   TextFieldColors,
};
use crate::Error;

const SANS_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");
const MONOSPACE_TTF: &[u8] = include_bytes!("assets/fonts/RobotoMono-Medium.ttf");

const ABOUT_HTML: Option<&[u8]> = {
   #[cfg(netcanv_has_about_html)]
   {
      Some(include_bytes!(concat!(env!("OUT_DIR"), "/about.html")))
   }
   #[cfg(not(netcanv_has_about_html))]
   {
      None
   }
};

const CHEVRON_RIGHT_SVG: &[u8] = include_bytes!("assets/icons/chevron-right.svg");
const CHEVRON_DOWN_SVG: &[u8] = include_bytes!("assets/icons/chevron-down.svg");
const ERASER_SVG: &[u8] = include_bytes!("assets/icons/eraser.svg");
const MENU_SVG: &[u8] = include_bytes!("assets/icons/menu.svg");
const COPY_SVG: &[u8] = include_bytes!("assets/icons/copy.svg");
const DRAG_HORIZONTAL_SVG: &[u8] = include_bytes!("assets/icons/drag-horizontal.svg");
const INFO_SVG: &[u8] = include_bytes!("assets/icons/info.svg");
const ERROR_SVG: &[u8] = include_bytes!("assets/icons/error.svg");
const PEER_CLIENT_SVG: &[u8] = include_bytes!("assets/icons/peer-client.svg");
const PEER_HOST_SVG: &[u8] = include_bytes!("assets/icons/peer-host.svg");
const DARK_MODE_SVG: &[u8] = include_bytes!("assets/icons/dark-mode.svg");
const LIGHT_MODE_SVG: &[u8] = include_bytes!("assets/icons/light-mode.svg");
const TRANSLATE_SVG: &[u8] = include_bytes!("assets/icons/translate.svg");
const LEGAL_SVG: &[u8] = include_bytes!("assets/icons/legal.svg");
const WINDOW_CLOSE_SVG: &[u8] = include_bytes!("assets/icons/window-close.svg");
const WINDOW_PIN_SVG: &[u8] = include_bytes!("assets/icons/window-pin.svg");
const WINDOW_PINNED_SVG: &[u8] = include_bytes!("assets/icons/window-pinned.svg");

const BANNER_BASE_SVG: &[u8] = include_bytes!("assets/banner/base.svg");
#[allow(unused)] // This is unused in debug mode, which doesn't render the long shadow.
const BANNER_SHADOW_PNG: &[u8] = include_bytes!("assets/banner/shadow.png");

const LANGUAGES_FTL: phf::Map<&str, &str> = phf::phf_map! {
   "en-US" => include_str!("assets/i18n/en-US.ftl"),
   "pl" => include_str!("assets/i18n/pl.ftl"),
};

/// Returns whether the licensing information page is available.
pub fn has_license_page() -> bool {
   ABOUT_HTML.is_some()
}

/// Opens the licensing information page.
pub fn open_license_page() -> netcanv::Result<()> {
   if let Some(about_html) = &ABOUT_HTML {
      let mut license_file =
         tempfile::Builder::new().prefix("netcanv-about").suffix(".html").tempfile()?;
      license_file.write_all(about_html)?;
      let (_, path) = license_file.keep().map_err(|e| Error::FailedToPersistTemporaryFile {
         error: e.to_string(),
      })?;
      let url = Url::from_file_path(path)
         .expect("license page path wasn't absolute and couldn't be turned into a URL");
      webbrowser::open(url.as_ref()).map_err(|_| Error::CouldNotOpenWebBrowser)?;
      Ok(())
   } else {
      Err(Error::NoLicensingInformationAvailable)
   }
}

/// Icons for navigation.
pub struct NavigationIcons {
   pub menu: Image,
   pub copy: Image,
   pub drag_horizontal: Image,
}

/// Icons for status messages.
pub struct StatusIcons {
   pub info: Image,
   pub error: Image,
}

/// Icons for peer roles.
pub struct PeerIcons {
   pub client: Image,
   pub host: Image,
}

/// Icons for the lobby.
pub struct LobbyIcons {
   pub dark_mode: Image,
   pub light_mode: Image,
   pub translate: Image,
   pub legal: Image,
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
   pub lobby: LobbyIcons,

   // Generic
   pub navigation: NavigationIcons,
   pub status: StatusIcons,
   pub peer: PeerIcons,
   pub window: WindowIcons,
}

/// Banner layers.
pub struct Banner {
   pub base: Image,
   pub shadow: Image,
}

/// App assets. This constitutes fonts, color schemes, icons, and the like.
pub struct Assets {
   pub sans: Font,
   pub sans_bold: Font,
   pub monospace: Font,

   pub colors: ColorScheme,
   pub icons: Icons,
   pub banner: Banner,

   pub languages: LanguageCodes,
   pub language: Language,
   pub tr: Strings,
}

impl Assets {
   /// Loads an SVG file to a texture.
   pub fn load_svg(renderer: &mut Backend, data: &[u8]) -> Image {
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

   /// Loads an image file into a texture.
   #[allow(unused)] // This is unused in debug mode, which doesn't render the long shadow.
   fn load_image(renderer: &mut Backend, data: &[u8]) -> Image {
      let image = image::io::Reader::new(Cursor::new(data))
         .with_guessed_format()
         .expect("unknown image format")
         .decode()
         .expect("error while loading the image file")
         .to_rgba8();
      renderer.create_image_from_rgba(image.width(), image.height(), &image)
   }

   /// Loads the mapping from language names to language codes.
   fn load_languages() -> LanguageCodes {
      const LANGUAGE_NAMES_TOML: &str = include_str!("assets/i18n/language-names.toml");
      toml::de::from_str(LANGUAGE_NAMES_TOML).unwrap()
   }

   /// Loads the language provided in the argument, or if the argument is `None`, the one specified
   /// in the config.
   pub fn load_language(language_code: Option<&str>) -> netcanv::Result<Language> {
      let language_code =
         language_code.map(|x| x.to_owned()).unwrap_or_else(|| config().language.clone());
      let language = Language::load(
         &language_code,
         LANGUAGES_FTL.get(&language_code).ok_or_else(|| Error::TranslationsDoNotExist {
            language: language_code.to_owned(),
         })?,
      );
      let language = match language {
         Ok(language) => language,
         Err(error) => {
            tracing::error!("error while loading language:");
            tracing::error!("{}", error);
            return Err(Error::CouldNotLoadLanguage {
               language: language_code,
            });
         }
      };
      Ok(language)
   }

   /// Creates a new instance of Assets with the provided color scheme.
   pub fn new(renderer: &mut Backend, colors: ColorScheme) -> netcanv::Result<Self> {
      profiling::scope!("Assets::new");

      let language = Self::load_language(None)?;
      let tr = Strings::from_language(&language);
      Ok(Self {
         sans: renderer.create_font_from_memory(SANS_TTF, 14.0),
         sans_bold: renderer.create_font_from_memory(SANS_BOLD_TTF, 14.0),
         monospace: renderer.create_font_from_memory(MONOSPACE_TTF, 14.0),

         colors,
         icons: Icons {
            expand: ExpandIcons {
               expand: Self::load_svg(renderer, CHEVRON_RIGHT_SVG),
               shrink: Self::load_svg(renderer, CHEVRON_DOWN_SVG),
            },
            color_picker: ColorPickerIcons {
               eraser: Self::load_svg(renderer, ERASER_SVG),
            },
            lobby: LobbyIcons {
               dark_mode: Self::load_svg(renderer, DARK_MODE_SVG),
               light_mode: Self::load_svg(renderer, LIGHT_MODE_SVG),
               translate: Self::load_svg(renderer, TRANSLATE_SVG),
               legal: Self::load_svg(renderer, LEGAL_SVG),
            },
            navigation: NavigationIcons {
               menu: Self::load_svg(renderer, MENU_SVG),
               copy: Self::load_svg(renderer, COPY_SVG),
               drag_horizontal: Self::load_svg(renderer, DRAG_HORIZONTAL_SVG),
            },
            status: StatusIcons {
               info: Self::load_svg(renderer, INFO_SVG),
               error: Self::load_svg(renderer, ERROR_SVG),
            },
            peer: PeerIcons {
               client: Self::load_svg(renderer, PEER_CLIENT_SVG),
               host: Self::load_svg(renderer, PEER_HOST_SVG),
            },
            window: WindowIcons {
               close: Self::load_svg(renderer, WINDOW_CLOSE_SVG),
               pin: Self::load_svg(renderer, WINDOW_PIN_SVG),
               pinned: Self::load_svg(renderer, WINDOW_PINNED_SVG),
            },
         },
         banner: Banner {
            base: Self::load_svg(renderer, BANNER_BASE_SVG).colorized(Color::WHITE),
            shadow: {
               #[cfg(not(debug_assertions))]
               {
                  Self::load_image(renderer, BANNER_SHADOW_PNG)
               }
               #[cfg(debug_assertions)]
               {
                  // NOTE: The shadow is disabled on debug mode because it slows down loading times
                  // significantly, and we don't have async PNG loading yet.
                  renderer.create_image_from_rgba(1, 1, &[0, 0, 0, 0])
               }
            },
         },

         languages: Self::load_languages(),
         language,
         tr,
      })
   }

   /// Reloads the language saved in the config file.
   pub fn reload_language(&mut self) -> netcanv::Result<()> {
      let language = Self::load_language(None)?;
      let tr = Strings::from_language(&language);
      self.language = language;
      self.tr = tr;
      Ok(())
   }
}

pub struct LanguageCodes(Vec<(String, String)>);

impl<'de> Deserialize<'de> for LanguageCodes {
   fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
   where
      D: serde::Deserializer<'de>,
   {
      struct MapVisitor;

      impl<'de> Visitor<'de> for MapVisitor {
         type Value = LanguageCodes;

         fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "language code mappings")
         }

         fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
         where
            A: serde::de::MapAccess<'de>,
         {
            let mut codes = Vec::new();
            while let Some((key, value)) = map.next_entry()? {
               codes.push((key, value));
            }
            Ok(LanguageCodes(codes))
         }
      }

      deserializer.deserialize_map(MapVisitor)
   }
}

impl Deref for LanguageCodes {
   type Target = [(String, String)];

   fn deref(&self) -> &Self::Target {
      &self.0
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
   pub toolbar: ToolbarColors,
   pub drag_handle: Color,

   pub lobby: LobbyColors,
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
         drag_handle: gray_60,
         toolbar: ToolbarColors {
            position_highlight: blue_50,
         },

         lobby: LobbyColors {
            background: blue_50,
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
