use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Cli {
   /// Dump a Chromium .json trace to the given file.
   #[clap(long)]
   pub trace: Option<PathBuf>,

   #[clap(flatten)]
   pub render: crate::backend::cli::RendererCli,
}
