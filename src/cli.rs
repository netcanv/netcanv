use std::path::PathBuf;

#[derive(clap::Parser)]
pub struct Cli {
   /// Dump verbose logs to the given file.
   #[clap(long)]
   pub log: Option<PathBuf>,

   #[clap(flatten)]
   pub render: crate::backend::cli::RendererCli,
}
