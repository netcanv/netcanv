//! Window content wrappers for the window manager.

use super::{WindowContent, WindowContentArgs};

/// Draws a gray, panel background below a window.
///
/// Create using [`WindowContent::background`].
pub struct Background<C, D>
where
   C: WindowContent<Data = D>,
{
   inner: C,
}

impl<C, D> WindowContent for Background<C, D>
where
   C: WindowContent<Data = D>,
{
   type Data = D;

   fn process(&mut self, mut args: WindowContentArgs, data: &mut Self::Data) {
      let WindowContentArgs { ui, assets, .. } = &mut args;
      ui.fill_rounded(assets.colors.panel, 4.0);
      self.inner.process(args, data);
   }
}

pub trait WindowContentWrappers<C, D>
where
   C: WindowContent<Data = D>,
{
   /// Creates a window content wrapper that draws a gray background below the content.
   fn background(self) -> Background<C, D>;
}

impl<C, D> WindowContentWrappers<C, D> for C
where
   C: WindowContent<Data = D>,
{
   fn background(self) -> Background<C, D> {
      Background { inner: self }
   }
}
