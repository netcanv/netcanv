use std::path::PathBuf;

use clap::Subcommand;
use netcanv_protocol::relay::{RoomId, RoomIdError};

#[derive(clap::Parser)]
pub struct Cli {
   /// Dump a Chromium .json trace to the given file.
   #[clap(long)]
   pub trace: Option<PathBuf>,

   #[clap(flatten)]
   pub render: crate::backend::cli::RendererCli,

   #[command(subcommand)]
   pub command: Option<Commands>,
}

// Borrow checker complains about lifetimes when we use RoomId::try_from
// for validation, despite it being compatible type.
// So just wrap it in a function.
fn is_valid_room_id(value: &str) -> Result<RoomId, RoomIdError> {
   RoomId::try_from(value)
}

#[derive(Subcommand)]
pub enum Commands {
   /// Host room when started
   HostRoom,
   /// Join room when started
   JoinRoom {
      /// Room ID used for joining the room
      #[arg(short, long, value_parser = is_valid_room_id)]
      room_id: RoomId,
   },
}
