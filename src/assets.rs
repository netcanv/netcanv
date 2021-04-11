use skulpin::skia_safe::*;

use crate::ui::{ButtonColors, ExpandColors, ExpandIcons, TextFieldColors};
use crate::util::{new_rc_font, RcFont};

const SANS_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

const CHEVRON_RIGHT_SVG: &[u8] = include_bytes!("assets/icons/chevron-right.svg");
const CHEVRON_DOWN_SVG: &[u8] = include_bytes!("assets/icons/chevron-down.svg");
const INFO_SVG: &[u8] = include_bytes!("assets/icons/info.svg");
const ERROR_SVG: &[u8] = include_bytes!("assets/icons/error.svg");
const SAVE_SVG: &[u8] = include_bytes!("assets/icons/save.svg");
const DARK_MODE_SVG: &[u8] = include_bytes!("assets/icons/dark-mode.svg");
const LIGHT_MODE_SVG: &[u8] = include_bytes!("assets/icons/light-mode.svg");

pub struct ColorScheme {
    pub text: Color,
    pub panel: Color,
    pub panel2: Color,
    pub separator: Color,
    pub error: Color,

    pub button: ButtonColors,
    pub tool_button: ButtonColors,
    pub expand: ExpandColors,
    pub slider: Color,
    pub text_field: TextFieldColors,

    pub titlebar: TitlebarColors
}

pub struct StatusIcons {
    pub info: Image,
    pub error: Image,
}

pub struct FileIcons {
    pub save: Image,
}

pub struct ColorSwitcherIcons {
    pub dark: Image,
    pub light: Image
}

pub struct Icons {
    pub expand: ExpandIcons,
    pub status: StatusIcons,
    pub file: FileIcons,
    pub color_switcher: ColorSwitcherIcons
}

pub struct Assets {
    pub sans: RcFont,
    pub sans_bold: RcFont,

    pub colors: ColorScheme,
    pub icons: Icons,

    pub dark_mode: bool
}

impl Assets {
    fn load_icon(data: &[u8]) -> Image {
        use usvg::{FitTo, NodeKind, Tree};

        let tree = Tree::from_data(data, &Default::default()).expect("error while loading the SVG file");
        let size = match *tree.root().borrow() {
            NodeKind::Svg(svg) => svg.size,
            _ => panic!("the root node of the SVG is not <svg/>"),
        };
        let mut pixmap = tiny_skia::Pixmap::new(size.width() as u32, size.height() as u32).unwrap();
        resvg::render(&tree, FitTo::Original, pixmap.as_mut());

        let image_info = ImageInfo::new(
            (size.width() as i32, size.height() as i32),
            ColorType::RGBA8888,
            AlphaType::Premul,
            ColorSpace::new_srgb(),
        );
        let stride = pixmap.width() as usize * 4;
        Image::from_raster_data(&image_info, Data::new_copy(pixmap.data()), stride).unwrap()
    }

    pub fn new(colors: ColorScheme) -> Self {
        Self {
            sans: new_rc_font(SANS_TTF, 14.0),
            sans_bold: new_rc_font(SANS_BOLD_TTF, 14.0),
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
                    light: Self::load_icon(LIGHT_MODE_SVG)
                }
            },

            dark_mode: false
        }
    }
}

impl ColorScheme {
    pub fn light() -> Self {
        Self {
            text: Color::new(0xff000000),
            panel: Color::new(0xffeeeeee),
            panel2: Color::new(0xffffffff),
            separator: Color::new(0xff202020),
            error: Color::new(0xff7f0000),

            button: ButtonColors {
                outline: Color::new(0x40000000),
                text: Color::new(0xff000000),
                hover: Color::new(0x20000000),
                pressed: Color::new(0x50000000),
            },
            tool_button: ButtonColors {
                outline: Color::new(0x00000000),
                text: Color::new(0xff000000),
                hover: Color::new(0x20000000),
                pressed: Color::new(0x50000000),
            },
            slider: Color::new(0xff000000),
            expand: ExpandColors {
                icon: Color::new(0xff000000),
                text: Color::new(0xff000000),
                hover: Color::new(0x30000000),
                pressed: Color::new(0x60000000),
            },
            text_field: TextFieldColors {
                outline: Color::new(0xff808080),
                outline_focus: Color::new(0xff303030),
                fill: Color::new(0xffffffff),
                text: Color::new(0xff000000),
                text_hint: Color::new(0x7f000000),
                label: Color::new(0xff000000),
            },
            titlebar: TitlebarColors {
                titlebar: Color::new(0xffffffff),
                separator: Color::new(0x7f000000),
                text: Color::new(0xff000000),

                foreground_hover: Color::new(0xffeeeeee),
                button: Color::new(0xff000000)
            }
        }
    }

    pub fn dark() -> Self {
        Self {
            text: Color::new(0xffb7b7b7),
            panel: Color::new(0xff1f1f1f),
            panel2: Color::new(0xffffffff),
            separator: Color::new(0xff202020),
            error: Color::new(0xfffc9292),

            button: ButtonColors {
                outline: Color::new(0xff444444),
                text: Color::new(0xffd2d2d2),
                hover: Color::new(0x20ffffff),
                pressed: Color::new(0x10ffffff),
            },
            tool_button: ButtonColors {
                outline: Color::new(0x00000000),
                text: Color::new(0xffb7b7b7),
                hover: Color::new(0x20ffffff),
                pressed: Color::new(0x10ffffff),
            },
            slider: Color::new(0xff979797),
            expand: ExpandColors {
                icon: Color::new(0xffb7b7b7),
                text: Color::new(0xffb7b7b7),
                hover: Color::new(0x60ffffff),
                pressed: Color::new(0x30ffffff),
            },
            text_field: TextFieldColors {
                outline: Color::new(0xff595959),
                outline_focus: Color::new(0xff9a9a9a),
                fill: Color::new(0xff383838),
                text: Color::new(0xffd5d5d5),
                text_hint: Color::new(0x7f939393),
                label: Color::new(0xffd5d5d5),
            },
            titlebar: TitlebarColors {
                titlebar: Color::new(0xff383838),
                separator: Color::new(0x7f939393),
                text: Color::new(0xffd5d5d5),

                foreground_hover: Color::new(0xff1f1f1f),
                button: Color::new(0xffb7b7b7)
            }
        }
    }
}

pub struct TitlebarColors {
    pub titlebar: Color,
    pub separator: Color,
    pub text: Color,

    pub foreground_hover: Color,
    pub button: Color
}

#[cfg(target_family = "unix")]
use winit::platform::unix::*;

#[cfg(target_family = "unix")]
fn winit_argb_from_skia_color(color: Color) -> ARGBColor {
    ARGBColor {
        a: color.a(),
        r: color.r(),
        g: color.g(),
        b: color.b(),
    }
}

#[cfg(target_family = "unix")]
impl Theme for ColorScheme {
    fn element_color(&self, element: Element, window_active: bool) -> ARGBColor {
        match element {
            Element::Bar => winit_argb_from_skia_color(self.titlebar.titlebar),
            Element::Separator => winit_argb_from_skia_color(self.titlebar.separator),
            Element::Text => winit_argb_from_skia_color(self.titlebar.text),
        }
    }

    fn button_color(&self, button: Button, state: ButtonState, foreground: bool, _window_active: bool) -> ARGBColor {
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