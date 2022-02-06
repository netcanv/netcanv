//! The `Save to file` action.

use instant::{Duration, Instant};

use native_dialog::FileDialog;

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
         ..
      }: ActionArgs,
   ) -> netcanv::Result<()> {
      match FileDialog::new()
         .add_filter(&assets.tr.fd_png_file, &["png"])
         .add_filter(&assets.tr.fd_netcanv_canvas, &["netcanv", "toml"])
         .show_save_single_file()
      {
         Ok(Some(path)) => paint_canvas.save(Some(&path))?,
         Ok(None) => (),
         Err(error) => anyhow::bail!(error),
      }
      Ok(())
   }

   fn process(&mut self, ActionArgs { paint_canvas, .. }: ActionArgs) -> netcanv::Result<()> {
      if paint_canvas.filename().is_some() && self.last_autosave.elapsed() > Self::AUTOSAVE_INTERVAL
      {
         log::info!("autosaving chunks");
         paint_canvas.save(None)?;
         log::info!("autosave complete");
         self.last_autosave = Instant::now();
      }
      Ok(())
   }
}
