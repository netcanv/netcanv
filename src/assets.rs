use crate::util::{RcFont, new_rc_font};

const SANS_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &'static [u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

pub struct Assets {
    pub sans: RcFont,
    pub sans_bold: RcFont,
}

impl Assets {

    pub fn new() -> Self {
        Self {
            sans: new_rc_font(SANS_TTF, 14.0),
            sans_bold: new_rc_font(SANS_BOLD_TTF, 14.0),
        }
    }

}
