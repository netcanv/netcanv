use std::path::PathBuf;

use clap::Subcommand;
use netcanv_protocol::relay::RoomId;

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

#[derive(Subcommand)]
pub enum Commands {
   /// Host room when started
   HostRoom {
      /// Nickname
      #[arg(long)]
      nickname: Option<String>,
      /// Relay server address
      #[arg(long)]
      relay: Option<String>,
   },
   /// Join room when started
   JoinRoom {
      /// Nickname
      #[arg(long)]
      nickname: Option<String>,
      /// Relay server address
      #[arg(long)]
      relay: Option<String>,
      /// Room ID used for joining the room
      #[arg(short, long, value_parser = clap::value_parser!(RoomId))]
      room_id: RoomId,
   },
}
