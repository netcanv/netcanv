use skulpin::skia_safe::Color;

use crate::ui::TextFieldColors;
use crate::util::{RcFont, new_rc_font};

const SANS_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

pub struct ColorScheme {
    pub text: Color,
    pub panel: Color,
    pub panel2: Color,
    pub separator: Color,
    pub slider: Color,
    pub text_field: TextFieldColors,
}

pub struct Assets {
    pub sans: RcFont,
    pub sans_bold: RcFont,

    pub colors: ColorScheme,
}

impl Assets {

    pub fn new(colors: ColorScheme) -> Self {
        Self {
            sans: new_rc_font(SANS_TTF, 14.0),
            sans_bold: new_rc_font(SANS_BOLD_TTF, 14.0),
            colors,
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
            slider: Color::new(0xff000000),
            text_field: TextFieldColors {
                fill: Color::new(0xffffffff),
                outline: Color::new(0xff303030),
                text: Color::new(0xff000000),
                text_hint: Color::new(0x7f000000),
                label: Color::new(0xff000000),
            },
        }
    }

}
