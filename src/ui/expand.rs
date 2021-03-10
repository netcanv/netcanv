use skulpin::skia_safe::*;

use crate::ui::*;

pub struct Expand {
    expanded: bool,
}

pub struct ExpandIcons {
    pub expand: Image,
    pub shrink: Image,
}

pub struct ExpandColors {
    pub text: Color,
    pub icon: Color,
    pub highlight: Color,
    pub pressed: Color,
}

impl Expand {

    pub fn new(expanded: bool) -> Self {
        Self {
            expanded,
        }
    }

    pub fn process(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        label: &str,
        font_size: f32,
        icons: &ExpandIcons,
        colors: &ExpandColors,
    ) {
        let icon =
            if self.expanded { &icons.shrink }
            else { &icons.expand };
        let height = icon.height() as f32;

        ui.push_group((ui.width(), height), Layout::Horizontal);

        ui.icon(canvas, icon, colors.icon, Some((height, height)));

        ui.pop_group();
    }

}
