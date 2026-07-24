use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub scrape_concurrency: u8,
    pub metadata_language: String,
    pub nfo_format: String,
    pub scan_excluded_folders: Vec<String>,
    pub rename_auto_after_scrape: bool,
    pub rename_create_season_folders: bool,
    pub appearance: String,
    pub api_keys: ApiKeysConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApiKeysConfig {
    pub tmdb: String,
    pub tvdb: String,
    pub omdb: String,
    #[serde(default)]
    pub bangumi: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scrape_concurrency: 4,
            metadata_language: "zh-CN".into(),
            nfo_format: "kodi".into(),
            scan_excluded_folders: vec![
                "NCOP&NCED".into(),
                "PV".into(),
                "menu".into(),
                "SP".into(),
                "Extras".into(),
                "Specials".into(),
                ".actors".into(),
            ],
            rename_auto_after_scrape: false,
            rename_create_season_folders: false,
            appearance: "system".into(),
            api_keys: ApiKeysConfig::default(),
        }
    }
}

pub struct ConfigStore {
    path: PathBuf,
    pub config: AppConfig,
}

impl ConfigStore {
    pub fn load_or_default(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let config = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            toml::from_str(&raw)?
        } else {
            let config = AppConfig::default();
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            let raw = toml::to_string_pretty(&config)?;
            fs::write(&path, raw)?;
            config
        };
        Ok(Self { path, config })
    }

    pub fn save(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw = toml::to_string_pretty(&self.config)?;
        let tmp = self.path.with_extension("toml.tmp");
        fs::write(&tmp, &raw)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}
