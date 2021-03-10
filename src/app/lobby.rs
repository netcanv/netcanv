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

    join_expand: Expand,
    host_expand: Expand,
}

impl State {

    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            ui: Ui::new(),
            nickname_field: TextField::new(None),
            matchmaker_field: TextField::new(Some("netcanv.org")),
            room_id_field: TextField::new(None),
            join_expand: Expand::new(true),
            host_expand: Expand::new(false),
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

    fn process_menu(&mut self, canvas: &mut Canvas, input: &mut Input) -> Option<Box<dyn AppState>> {
        self.ui.push_group((self.ui.width(), self.ui.remaining_height()), Layout::Vertical);

        let textfield = TextFieldArgs {
            width: 160.0,
            colors: &self.assets.colors.text_field,
            hint: None,
        };
        let expand = ExpandArgs {
            label: "",
            font_size: 22.0,
            icons: &self.assets.icons.expand,
            colors: &self.assets.colors.expand,
        };

        // nickname, matchmaker
        self.ui.push_group((self.ui.width(), TextField::labelled_height(&self.ui)), Layout::Horizontal);
        self.nickname_field.process_with_label(&mut self.ui, canvas, input, "Nickname", TextFieldArgs {
            hint: Some("Name shown to others"),
            .. textfield
        });
        self.ui.space(16.0);
        self.matchmaker_field.process_with_label(&mut self.ui, canvas, input, "Matchmaker", TextFieldArgs {
            hint: Some("IP address"),
            .. textfield
        });
        self.ui.pop_group();
        self.ui.space(32.0);

        // join room
        if self.join_expand.process(&mut self.ui, canvas, input, ExpandArgs {
            label: "Join an existing room",
            .. expand
        })
            .mutually_exclude(&mut self.host_expand)
            .expanded()
        {
            self.ui.push_group(self.ui.remaining_size(), Layout::Vertical);
            self.ui.offset((32.0, 8.0));

            self.ui.paragraph(canvas, self.assets.colors.text, AlignH::Left, None, &[
                "Ask your friend for the Room ID",
                "and enter it into the text field below."
            ]);
            self.ui.space(16.0);
            self.room_id_field.process_with_label(&mut self.ui, canvas, input, "Room ID", TextFieldArgs {
                hint: Some("4–6 digits"),
                .. textfield
            });

            self.ui.fit();
            self.ui.pop_group();
        }
        self.ui.space(16.0);

        // host room
        if self.host_expand.process(&mut self.ui, canvas, input, ExpandArgs {
            label: "Host a new room",
            .. expand
        })
            .mutually_exclude(&mut self.join_expand)
            .expanded()
        {
            self.ui.push_group(self.ui.remaining_size(), Layout::Vertical);
            self.ui.offset((32.0, 8.0));

            self.ui.paragraph(canvas, self.assets.colors.text, AlignH::Left, None, &[
                "Click 'Host' and share the Room ID",
                "with your friends.",
            ]);

            self.ui.fit();
            self.ui.pop_group();
        }

        self.ui.pop_group();

        chain_focus(input, &mut [
            &mut self.nickname_field,
            &mut self.matchmaker_field,
            &mut self.room_id_field,
        ]);

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
