//! Translatable strings.

use netcanv_i18n::{Formatted, FromLanguage};

#[derive(FromLanguage)]
pub struct Strings {
   pub lobby_welcome: String,

   pub lobby_nickname: LabelledTextField,
   pub lobby_relay_server: LabelledTextField,

   pub lobby_join_a_room: ExpandWithDescription,
   pub lobby_room_id: LabelledTextField,
   pub lobby_join: String,

   pub lobby_host_a_new_room: ExpandWithDescription,
   pub lobby_host: String,
   pub lobby_host_from_file: String,

   pub connecting: String,
   pub error_nickname_must_not_be_empty: String,
   pub error_nickname_too_long: Formatted,
   pub error_invalid_room_id_length: Formatted,
   pub error_fatal: Formatted,

   pub switch_to_dark_mode: String,
   pub switch_to_light_mode: String,
   pub open_source_licenses: String,

   pub fd_supported_image_files: String,
   pub fd_netcanv_canvas: String,
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
