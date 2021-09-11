//! User configuration.

use std::path::PathBuf;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct LobbyConfig {
    pub nickname: String,
    pub matchmaker: String,
}

#[derive(Deserialize, Serialize)]
pub enum ColorScheme {
    Light,
    Dark,
}

#[derive(Deserialize, Serialize)]
pub struct UiConfig {
    pub color_scheme: ColorScheme,
}

#[derive(Deserialize, Serialize)]
pub struct UserConfig {
    pub lobby: LobbyConfig,
    pub ui: UiConfig,
}

impl UserConfig {
    pub fn config_dir() -> PathBuf {
        let project_dirs = ProjectDirs::from("", "", "NetCanv").expect("cannot determine config directories");
        project_dirs.config_dir().to_owned()
    }

    pub fn path() -> PathBuf {
        Self::config_dir().join("config.toml")
    }

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
            let config = match toml::from_str(&file) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("error while deserializing config file: {}", error);
                    eprintln!("falling back to default config");
                    Self::default()
                },
            };
            Ok(config)
        }
    }

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
                matchmaker: "localhost:62137".to_owned(),
            },
            ui: UiConfig {
                color_scheme: ColorScheme::Light,
            },
        }
    }
}
