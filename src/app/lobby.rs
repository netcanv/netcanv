use std::error::Error;
use std::fmt::Display;
use std::net::SocketAddr;

use skulpin::skia_safe::*;

use crate::app::{AppState, StateArgs, paint};
use crate::assets::Assets;
use crate::ui::*;
use crate::util::get_window_size;
use netcanv::net::{Message, Peer};

#[derive(Debug)]
enum Status {
    None,
    Info(String),
    Error(String),
}

impl<T: Error + Display> From<T> for Status {
    fn from(error: T) -> Self {
        Self::Error(format!("{}", error))
    }
}

pub struct State {
    assets: Assets,
    ui: Ui,

    // UI elements

    nickname_field: TextField,
    matchmaker_field: TextField,
    room_id_field: TextField,

    join_expand: Expand,
    host_expand: Expand,

    // net
    status: Status,
    net: Option<Net>,
}

/// connection stuff
// needed to yeet this into another struct because compiler complained:
// "something something mutable borrow something something you're an idiot something"
// is what it said idk i wasn't paying attention
struct Net {
    me: Peer,
}

impl State {

    pub fn new(assets: Assets) -> Self {
        Self {
            assets,
            ui: Ui::new(),
            nickname_field: TextField::new(None),
            matchmaker_field: TextField::new(Some("internetcanv.as:62137")),
            room_id_field: TextField::new(None),
            join_expand: Expand::new(true),
            host_expand: Expand::new(false),
            status: Status::None,
            net: None,
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

        let button = ButtonArgs {
            height: 32.0,
            colors: &self.assets.colors.button,
        };
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
        self.nickname_field.with_label(&mut self.ui, canvas, input, "Nickname", TextFieldArgs {
            hint: Some("Name shown to others"),
            .. textfield
        });
        self.ui.space(16.0);
        self.matchmaker_field.with_label(&mut self.ui, canvas, input, "Matchmaker", TextFieldArgs {
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
            self.ui.push_group((0.0, TextField::labelled_height(&self.ui)), Layout::Horizontal);
            self.room_id_field.with_label(&mut self.ui, canvas, input, "Room ID", TextFieldArgs {
                hint: Some("4–6 digits"),
                .. textfield
            });
            self.ui.offset((16.0, 16.0));
            if Button::with_text(&mut self.ui, canvas, input, button, "Join").clicked() {
                match Self::join_room(self.matchmaker_field.text(), self.room_id_field.text()) {
                    Ok(net) => {
                        self.net = Some(net);
                        self.status = Status::None;
                    },
                    Err(status) => self.status = status,
                }
            }
            self.ui.pop_group();

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
            self.ui.space(16.0);
            if Button::with_text(&mut self.ui, canvas, input, button, "Host").clicked() {
                match Self::host_room(self.matchmaker_field.text()) {
                    Ok(net) => {
                        self.net = Some(net);
                        self.status = Status::None;
                    },
                    Err(status) => self.status = status,
                }
            }

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

    fn process_status(&mut self, canvas: &mut Canvas) {
        if !matches!(self.status, Status::None) {
            self.ui.push_group((self.ui.width(), 24.0), Layout::Horizontal);
            let icon =
                match self.status {
                    Status::None => unreachable!(),
                    Status::Info(_) => &self.assets.icons.status.info,
                    Status::Error(_) => &self.assets.icons.status.error,
                };
            let color =
                match self.status {
                    Status::None => unreachable!(),
                    Status::Info(_) => self.assets.colors.text,
                    Status::Error(_) => self.assets.colors.error,
                };
            self.ui.icon(canvas, icon, color, Some((self.ui.height(), self.ui.height())));
            self.ui.space(8.0);
            self.ui.push_group((self.ui.remaining_width(), self.ui.height()), Layout::Freeform);
            let text =
                match &self.status {
                    Status::None => unreachable!(),
                    Status::Info(text) | Status::Error(text) => text,
                };
            self.ui.text(canvas, text, color, (AlignH::Left, AlignV::Middle));
            self.ui.pop_group();
            self.ui.pop_group();
        }
    }

    fn host_room(matchmaker_addr_str: &str) -> Result<Net, Status> {
        Ok(Net {
            me: Peer::host_room(matchmaker_addr_str)?,
        })
    }

    fn join_room(matchmaker_addr_str: &str, room_id_str: &str) -> Result<Net, Status> {
        if !matches!(room_id_str.len(), 4..=6) {
            return Err(Status::Error("Room ID must be a number with 4–6 digits".into()))
        }
        let room_id: u32 = room_id_str.parse()
            .map_err(|_| Status::Error("Room ID must be an integer".into()))?;
        Ok(Net {
            me: Peer::join_room(matchmaker_addr_str, room_id)?,
        })
    }

}

impl Net {

    fn poll(&mut self) -> Option<Status> {
        while let Some(message) = self.me.next_message() {
            match message {
                Message::Info(msg) => return Some(Status::Info(msg)),
                _ => ()
            }
        }
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

        if let Some(net) = &mut self.net {
            if let Some(status) = net.poll() {
                self.status = status;
            }
        }

        self.ui.begin(get_window_size(&coordinate_system_helper), Layout::Freeform);
        self.ui.set_font(self.assets.sans.clone());
        self.ui.set_font_size(14.0);

        self.ui.pad((64.0, 64.0));

        self.ui.push_group((self.ui.width(), 384.0), Layout::Vertical);
        self.ui.align((AlignH::Left, AlignV::Middle));
        self.process_header(canvas);
        self.ui.space(24.0);
        self.process_menu(canvas, input);
        self.ui.space(24.0);
        self.process_status(canvas);
        self.ui.pop_group();

        None
    }

}
