//! User configuration.

use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

/// Saved values of lobby text boxes.
#[derive(Deserialize, Serialize)]
pub struct LobbyConfig {
   pub nickname: String,
   pub matchmaker: String,
}

/// The color scheme variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ColorScheme {
   Light,
   Dark,
}

/// The position of the toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum ToolbarPosition {
   Left,
   Top,
   Bottom,
   Right,
}

impl Default for ToolbarPosition {
   fn default() -> Self {
      Self::Left
   }
}

/// UI-related configuration options.
#[derive(Deserialize, Serialize)]
pub struct UiConfig {
   pub color_scheme: ColorScheme,
   #[serde(default)]
   pub toolbar_position: ToolbarPosition,
}

/// A user `config.toml` file.
#[derive(Deserialize, Serialize)]
pub struct UserConfig {
   pub lobby: LobbyConfig,
   pub ui: UiConfig,
}

impl UserConfig {
   /// Returns the platform-specific configuration directory.
   pub fn config_dir() -> PathBuf {
      let project_dirs =
         ProjectDirs::from("", "", "NetCanv").expect("cannot determine config directories");
      project_dirs.config_dir().to_owned()
   }

   /// Returns the path to the `config.toml` file.
   pub fn path() -> PathBuf {
      Self::config_dir().join("config.toml")
   }

   /// Loads the `config.toml` file.
   ///
   /// If the `config.toml` doesn't exist, it's created with values inherited from
   /// `UserConfig::default`.
   pub fn load_or_create() -> anyhow::Result<Self> {
      let config_dir = Self::config_dir();
      let config_file = Self::path();
      std::fs::create_dir_all(config_dir)?;
      if !config_file.is_file() {
         let config = Self::default();
         config.save()?;
         Ok(config)
      } else {
         let file = std::fs::read_to_string(&config_file)?;
         let config: Self = match toml::from_str(&file) {
            Ok(config) => config,
            Err(error) => {
               eprintln!("error while deserializing config file: {}", error);
               eprintln!("falling back to default config");
               return Ok(Self::default());
            }
         };
         // Preemptively save the config to the disk if any new keys have been added.
         // I'm not sure if errors should be treated as fatal or not in this case.
         config.save()?;
         Ok(config)
      }
   }

   /// Saves the user configuration to the `config.toml` file.
   pub fn save(&self) -> anyhow::Result<()> {
      // Assumes that `config_dir` was already created in `load_or_create`.
      let config_file = Self::path();
      std::fs::write(&config_file, toml::to_string(self)?)?;
      Ok(())
   }
}

impl Default for UserConfig {
   fn default() -> Self {
      Self {
         lobby: LobbyConfig {
            nickname: "Anon".to_owned(),
            matchmaker: "localhost".to_owned(),
         },
         ui: UiConfig {
            color_scheme: ColorScheme::Light,
            toolbar_position: ToolbarPosition::Left,
         },
      }
   }
}
