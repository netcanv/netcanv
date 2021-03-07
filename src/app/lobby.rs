use skulpin::skia_safe::*;

use crate::app::{AppState, StateArgs, paint};
use crate::assets::Assets;
use crate::ui::*;
use crate::util::get_window_size;

pub struct State {
    assets: Assets,
    ui: Ui,
}

impl State {

    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            ui: Ui::new(),
        }
    }

    fn process_header(&mut self, canvas: &mut Canvas) {
        self.ui.push_group((self.ui.width(), 72.0), Layout::Vertical);

        self.ui.push_group((self.ui.width(), 56.0), Layout::Freeform);
        self.ui.set_font_size(48.0);
        self.ui.text(canvas, "NetCanv", self.assets.colors.text, (AlignH::Left, AlignV::Middle));
        self.ui.pop_group();

        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Freeform);
        self.ui.text(
            canvas,
            "Welcome! Host a room or join an existing one to start painting.",
            self.assets.colors.text,
            (AlignH::Left, AlignV::Middle),
        );
        self.ui.pop_group();

        self.ui.pop_group();
    }

    fn process_menu(&mut self, canvas: &mut Canvas, input: &Input) -> Option<Box<dyn AppState>> {
        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Freeform);

        self.ui.pop_group();

        None
    }

}

impl AppState for State {

    fn process(
        &mut self,
        StateArgs {
            canvas,
            coordinate_system_helper,
            input,
        }: StateArgs,
    ) -> Option<Box<dyn AppState>> {
        canvas.clear(self.assets.colors.panel);

        self.ui.begin(get_window_size(&coordinate_system_helper), Layout::Freeform);
        self.ui.set_font(self.assets.sans.clone());
        self.ui.set_font_size(14.0);

        self.ui.pad((64.0, 64.0));

        self.ui.push_group((self.ui.width(), 420.0), Layout::Vertical);
        self.ui.align((AlignH::Left, AlignV::Middle));
        self.process_header(canvas);
        self.ui.space(16.0);
        self.ui.pop_group();

        None
    }

}
