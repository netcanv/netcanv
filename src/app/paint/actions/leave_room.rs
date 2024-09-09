use crate::backend::{Backend, Image};
use crate::Assets;

use super::{Action, ActionArgs, ActionMessage};

pub struct LeaveTheRoomAction {
   icon: Image,
}

impl LeaveTheRoomAction {
   pub fn new(renderer: &mut Backend) -> Self {
      Self {
         icon: Assets::load_svg(
            renderer,
            include_bytes!("../../../assets/icons/exit-to-app.svg"),
         ),
      }
   }
}

impl Action for LeaveTheRoomAction {
   fn name(&self) -> &str {
      "leave-the-room"
   }

   fn icon(&self) -> &Image {
      &self.icon
   }

   fn perform(&mut self, _args: ActionArgs) -> netcanv::Result<Option<ActionMessage>> {
      Ok(Some(ActionMessage::LeaveTheRoom))
   }

   fn process(&mut self, ActionArgs { .. }: ActionArgs) -> netcanv::Result<()> {
      Ok(())
   }
}
