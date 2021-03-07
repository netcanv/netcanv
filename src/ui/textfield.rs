// quite simplistic text field implementation.

use skulpin::skia_safe::*;

use crate::ui::*;

pub struct TextField {
    text: Vec<char>,
    text_utf8: String,
    focused: bool,
}

#[derive(Clone)]
pub struct TextFieldColors {
    pub outline: Color,
    pub fill: Color,
    pub text: Color,
    pub text_hint: Color,
    pub label: Color,
}

impl TextField {

    pub fn new(initial_text: Option<&str>) -> Self {
        let text_utf8: String = initial_text.unwrap_or("").into();
        Self {
            text: text_utf8.chars().collect(),
            text_utf8,
            focused: false,
        }
    }

    fn update_utf8(&mut self) {
        self.text_utf8 = self.text.iter().collect();
    }

    pub fn height(ui: &Ui) -> f32 {
        f32::round(16.0/7.0 * ui.font_size())
    }

    pub fn process(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        width: f32,
        colors: &TextFieldColors,
        hint: Option<&str>,
    ) {
        ui.push_group((width, Self::height(ui)), Layout::Freeform);

        // rendering: box
        ui.draw_on_canvas(canvas, |canvas| {
            let mut paint = Paint::new(Color4f::from(colors.fill), None);
            paint.set_anti_alias(true);
            let mut rrect = RRect::new_rect_xy(&Rect::from_point_and_size((0.0, 0.0), ui.size()), 4.0, 4.0);
            canvas.draw_rrect(rrect, &paint);
            paint.set_color(colors.outline);
            paint.set_style(paint::Style::Stroke);
            rrect.offset((0.5, 0.5));
            canvas.draw_rrect(rrect, &paint);
        });

        // rendering: text
        ui.push_group(ui.size(), Layout::Freeform);
        ui.pad((16.0, 0.0));
        canvas.save();
        ui.clip(canvas);

        // render hint
        if hint.is_some() && self.text.len() == 0 {
            ui.text(canvas, hint.unwrap(), colors.text_hint, (AlignH::Left, AlignV::Middle));
        }
        ui.text(canvas, &self.text_utf8, colors.text, (AlignH::Left, AlignV::Middle));

        canvas.restore();
        ui.pop_group();

        ui.pop_group();
    }

    pub fn labelled_height(ui: &Ui) -> f32 {
        16.0 + TextField::height(ui)
    }

    pub fn process_with_label(
        &mut self,
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        width: f32,
        colors: &TextFieldColors,
        label: &str,
        hint: Option<&str>,
    ) {
        ui.push_group((width, Self::labelled_height(ui)), Layout::Vertical);

        // label
        ui.push_group((width, 16.0), Layout::Freeform);
        ui.text(canvas, label, colors.label, (AlignH::Left, AlignV::Top));
        ui.pop_group();

        // field
        self.process(ui, canvas, input, width, colors, hint);

        ui.pop_group();
    }

    pub fn text<'a>(&'a self) -> &'a str {
        &self.text_utf8
    }

}
