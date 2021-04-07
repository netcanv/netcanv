use skulpin::skia_safe::*;

use crate::ui::*;

pub struct Button;

pub struct ButtonColors {
    pub outline: Color,
    pub text: Color,
    pub hover: Color,
    pub pressed: Color,
}

#[derive(Clone, Copy)]
pub struct ButtonArgs<'a> {
    pub height: f32,
    pub colors: &'a ButtonColors,
}

pub struct ButtonProcessResult {
    clicked: bool,
}

impl Button {
    pub fn process(
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        ButtonArgs { height, colors }: ButtonArgs,
        width_hint: Option<f32>,
        extra: impl FnOnce(&mut Ui, &mut Canvas),
    ) -> ButtonProcessResult {
        // horizontal because we need to fit() later
        ui.push_group((width_hint.unwrap_or(0.0), height), Layout::Horizontal);

        extra(ui, canvas);
        ui.fit();

        let mut clicked = false;
        ui.outline(canvas, colors.outline, 1.0);
        if ui.has_mouse(input) {
            let fill_color = if input.mouse_button_is_down(MouseButton::Left) {
                colors.pressed
            } else {
                colors.hover
            };
            ui.fill(canvas, fill_color);
            clicked = input.mouse_button_just_released(MouseButton::Left);
        }

        ui.pop_group();

        ButtonProcessResult { clicked }
    }

    pub fn with_text(
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        args: ButtonArgs,
        text: &str,
    ) -> ButtonProcessResult {
        Self::process(ui, canvas, input, args, None, |ui, canvas| {
            let text_width = ui.text_size(text).0;
            let padding = args.height;
            ui.push_group((text_width + padding, ui.height()), Layout::Freeform);
            ui.text(canvas, text, args.colors.text, (AlignH::Center, AlignV::Middle));
            ui.pop_group();
        })
    }

    pub fn with_icon(
        ui: &mut Ui,
        canvas: &mut Canvas,
        input: &Input,
        args: ButtonArgs,
        icon: &Image,
    ) -> ButtonProcessResult {
        Self::process(ui, canvas, input, args, Some(args.height), |ui, canvas| {
            ui.icon(canvas, icon, args.colors.text, Some((args.height, args.height)));
        })
    }
}

impl ButtonProcessResult {
    pub fn clicked(self) -> bool {
        self.clicked
    }
}
