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
    pub hover: Color,
    pub pressed: Color,
}

#[derive(Clone, Copy)]
pub struct ExpandArgs<'a, 'b, 'c> {
    pub label: &'a str,
    pub font_size: f32,
    pub icons: &'b ExpandIcons,
    pub colors: &'c ExpandColors,
}

impl Expand {

    pub fn new(expanded: bool) -> Self {
        Self {
            expanded,
        }
    }

    #[must_use]
    pub fn process(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        ExpandArgs { label, font_size, icons, colors }: ExpandArgs,
    ) -> bool {
        let icon =
            if self.expanded { &icons.shrink }
            else { &icons.expand };
        let height = icon.height() as f32;

        ui.push_group((ui.width(), height), Layout::Horizontal);

        ui.icon(canvas, icon, colors.icon, Some((height, height)));
        ui.space(8.0);
        ui.push_group((ui.remaining_width(), ui.height()), Layout::Freeform);
        ui.set_font_size(font_size);
        ui.text(canvas, label, colors.text, (AlignH::Left, AlignV::Middle));
        ui.pop_group();

        ui.pop_group();

        self.expanded
    }

}

