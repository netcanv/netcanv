use skulpin::skia_safe::*;

use crate::ui::{ExpandColors, ExpandIcons, TextFieldColors};
use crate::util::{RcFont, new_rc_font};

const SANS_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Medium.ttf");
const SANS_BOLD_TTF: &[u8] = include_bytes!("assets/fonts/Barlow-Bold.ttf");

const CHEVRON_RIGHT_SVG: &[u8] = include_bytes!("assets/icons/chevron-right.svg");
const CHEVRON_DOWN_SVG: &[u8] = include_bytes!("assets/icons/chevron-down.svg");

pub struct ColorScheme {
    pub text: Color,
    pub panel: Color,
    pub panel2: Color,
    pub separator: Color,
    pub slider: Color,
    pub expand: ExpandColors,
    pub text_field: TextFieldColors,
}

pub struct Icons {
    pub expand: ExpandIcons,
}

pub struct Assets {
    pub sans: RcFont,
    pub sans_bold: RcFont,

    pub colors: ColorScheme,
    pub icons: Icons,
}

impl Assets {

    fn load_icon(data: &[u8]) -> Image {
        use usvg::{FitTo, NodeKind, Tree};

        let tree = Tree::from_data(data, &Default::default())
            .expect("error while loading the SVG file");
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
            },
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
            expand: ExpandColors {
                icon: Color::new(0xff000000),
                text: Color::new(0xff000000),
                hover: Color::new(0x4f000000),
                pressed: Color::new(0x7f000000),
            },
            text_field: TextFieldColors {
                outline: Color::new(0xff808080),
                outline_focus: Color::new(0xff303030),
                fill: Color::new(0xffffffff),
                text: Color::new(0xff000000),
                text_hint: Color::new(0x7f000000),
                label: Color::new(0xff000000),
            },
        }
    }

}
