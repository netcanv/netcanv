//! The `Save to file` action.

use web_time::{Duration, Instant};

use rfd::FileDialog;

use crate::assets::Assets;
use crate::backend::{Backend, Image};

use super::{Action, ActionArgs};

pub struct SaveToFileAction {
   icon: Image,
   last_autosave: Instant,
}

impl SaveToFileAction {
   const AUTOSAVE_INTERVAL: Duration = Duration::from_secs(60);

   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icon: Assets::load_svg(renderer, include_bytes!("../../../assets/icons/save.svg")),
         last_autosave: Instant::now(),
      }
   }
}

impl Action for SaveToFileAction {
   fn name(&self) -> &str {
      "save-to-file"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }

   fn perform(
      &mut self,
      ActionArgs {
         assets,
         paint_canvas,
         project_file,
         renderer,
         ..
      }: ActionArgs,
   ) -> netcanv::Result<()> {
      if let Some(path) = FileDialog::new()
         .add_filter(&assets.tr.fd_png_file, &["png"])
         .add_filter(&assets.tr.fd_netcanv_canvas, &["netcanv", "toml"])
         .save_file()
      {
         project_file.save(renderer, Some(&path), paint_canvas)?
      }
      Ok(())
   }

   fn process(
      &mut self,
      ActionArgs {
         paint_canvas,
         project_file,
         renderer,
         ..
      }: ActionArgs,
   ) -> netcanv::Result<()> {
      if project_file.filename().is_some() && self.last_autosave.elapsed() > Self::AUTOSAVE_INTERVAL
      {
         tracing::info!("autosaving chunks");
         project_file.save(renderer, None, paint_canvas)?;
         tracing::info!("autosave complete");
         self.last_autosave = Instant::now();
      }
      Ok(())
   }
}
