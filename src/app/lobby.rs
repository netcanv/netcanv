use skulpin::skia_safe::*;

use crate::app::{AppState, StateArgs, paint};
use crate::assets::Assets;
use crate::ui::*;
use crate::util::get_window_size;

pub struct State {
    assets: Assets,
    ui: Ui,

    nickname_field: TextField,
    matchmaker_field: TextField,
    room_id_field: TextField,
}

impl State {

    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            ui: Ui::new(),
            nickname_field: TextField::new(None),
            matchmaker_field: TextField::new(Some("netcanv.org")),
            room_id_field: TextField::new(None),
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
        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Vertical);

        // nickname, matchmaker
        self.ui.push_group((self.ui.width(), TextField::labelled_height(&self.ui)), Layout::Horizontal);
        self.nickname_field.process_with_label(
            &mut self.ui,
            canvas,
            input,
            160.0,
            &self.assets.colors.text_field,
            "Nickname",
            Some("Name shown to others"),
        );
        self.ui.space(16.0);
        self.matchmaker_field.process_with_label(
            &mut self.ui,
            canvas,
            input,
            160.0,
            &self.assets.colors.text_field,
            "Matchmaker",
            Some("IP address and port"),
        );
        self.ui.pop_group();

        self.ui.icon(canvas, &self.assets.icons.chevron_down, Color::BLACK, None);
        self.ui.icon(canvas, &self.assets.icons.chevron_down, Color::WHITE, None);
        self.ui.icon(canvas, &self.assets.icons.chevron_down, Color::CYAN, None);

        self.ui.pop_group();

        chain_focus(input, &mut [&mut self.nickname_field, &mut self.matchmaker_field]);

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
        self.ui.space(24.0);
        self.process_menu(canvas, input);
        self.ui.pop_group();

        None
    }

}
