use clap::{value_parser, Subcommand};
use netcanv_protocol::relay::RoomId;
use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Cli {
   /// Dump a Chromium .json trace to the given file.
   #[clap(long)]
   pub trace: Option<PathBuf>,

   #[clap(flatten)]
   pub render: crate::backend::cli::RendererCli,

   #[command(subcommand)]
   pub command: Option<Commands>,

   /// Sets the default zoom level (range: -8..20).
   #[clap(long)]
   #[arg(allow_negative_numbers = true, value_parser = value_parser!(i8).range(-8..20))]
   pub zoom_level: Option<i8>,
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
