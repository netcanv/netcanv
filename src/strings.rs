//! Translatable strings.

use netcanv_i18n::{Formatted, FromLanguage, Map};

#[derive(FromLanguage)]
pub struct Strings {
   //
   // General nomenclature
   //
   pub room_id: String,

   //
   // Lobby
   //
   pub lobby_welcome: String,

   pub lobby_nickname: LabelledTextField,
   pub lobby_relay_server: LabelledTextField,

   pub lobby_join_a_room: ExpandWithDescription,
   pub lobby_room_id: LabelledTextField,
   pub lobby_join: String,

   pub lobby_host_a_new_room: ExpandWithDescription,
   pub lobby_host: String,
   pub lobby_host_from_file: String,

   pub switch_to_dark_mode: String,
   pub switch_to_light_mode: String,
   pub open_source_licenses: String,

   pub connecting: String,

   //
   // Paint
   //
   pub paint_welcome_host: String,

   pub unknown_host: String,
   pub you_are_the_host: String,
   pub someone_is_your_host: String,
   pub room_id_copied: String,

   pub someone_joined_the_room: Formatted,
   pub someone_left_the_room: Formatted,
   pub someone_is_now_hosting_the_room: Formatted,
   pub you_are_now_hosting_the_room: String,

   pub tool: Map<String>,
   pub brush_thickness: String,

   pub action: Map<String>,

   //
   // Color picker
   //
   pub click_to_edit_color: String,
   pub eraser: String,
   pub rgb_hex_code: String,

   //
   // File dialogs
   //
   pub fd_supported_image_files: String,
   pub fd_png_file: String,
   pub fd_netcanv_canvas: String,

   //
   // Errors
   //
   pub error: Formatted,
   pub error_fatal: Formatted,
   pub error_nickname_must_not_be_empty: String,
   pub error_nickname_too_long: Formatted,
   pub error_invalid_room_id_length: Formatted,
   pub error_while_performing_action: Formatted,
   pub error_while_processing_action: Formatted,
}

#[derive(FromLanguage)]
pub struct ExpandWithDescription {
   pub title: String,
   pub description: String,
}

#[derive(FromLanguage)]
pub struct LabelledTextField {
   pub label: String,
   pub hint: String,
}
