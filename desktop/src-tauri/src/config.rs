use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub scrape_concurrency: u8,
    pub metadata_language: String,
    pub nfo_format: String,
    pub scan_excluded_folders: Vec<String>,
    pub rename_auto_after_scrape: bool,
    pub rename_create_season_folders: bool,
    #[serde(default = "default_movie_folder_template")]
    pub rename_movie_folder_template: String,
    #[serde(default = "default_movie_file_template")]
    pub rename_movie_file_template: String,
    #[serde(default = "default_tv_show_folder_template")]
    pub rename_tv_show_folder_template: String,
    #[serde(default = "default_season_folder_template")]
    pub rename_season_folder_template: String,
    #[serde(default = "default_episode_file_template")]
    pub rename_episode_file_template: String,
    pub appearance: String,
    /// Accent axis id: indigo | teal | sky | slate (kaigua presets).
    #[serde(default = "default_accent")]
    pub accent: String,
    #[serde(default = "default_tray_enabled")]
    pub tray_enabled: bool,
    /// UI language: zh-Hans | en | ja (I18N surface language).
    #[serde(default = "default_ui_locale")]
    pub ui_locale: String,
    pub api_keys: ApiKeysConfig,
}

fn default_tray_enabled() -> bool {
    true
}

fn default_accent() -> String {
    "indigo".into()
}

fn default_ui_locale() -> String {
    "zh-Hans".into()
}

fn default_movie_folder_template() -> String {
    renamer::TemplateEngine::MOVIE_FOLDER.into()
}
fn default_movie_file_template() -> String {
    renamer::TemplateEngine::MOVIE_FILE.into()
}
fn default_tv_show_folder_template() -> String {
    renamer::TemplateEngine::TV_SHOW_FOLDER.into()
}
fn default_season_folder_template() -> String {
    renamer::TemplateEngine::SEASON_FOLDER.into()
}
fn default_episode_file_template() -> String {
    renamer::TemplateEngine::EPISODE_FILE.into()
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
            rename_movie_folder_template: default_movie_folder_template(),
            rename_movie_file_template: default_movie_file_template(),
            rename_tv_show_folder_template: default_tv_show_folder_template(),
            rename_season_folder_template: default_season_folder_template(),
            rename_episode_file_template: default_episode_file_template(),
            appearance: "system".into(),
            accent: default_accent(),
            tray_enabled: true,
            ui_locale: default_ui_locale(),
            api_keys: ApiKeysConfig::default(),
        }
    }
}

impl AppConfig {
    pub fn rename_templates(&self) -> renamer::RenameTemplates {
        renamer::RenameTemplates {
            movie_folder: self.rename_movie_folder_template.clone(),
            movie_file: self.rename_movie_file_template.clone(),
            tv_show_folder: self.rename_tv_show_folder_template.clone(),
            season_folder: self.rename_season_folder_template.clone(),
            episode_file: self.rename_episode_file_template.clone(),
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
