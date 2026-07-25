use std::path::PathBuf;
use std::sync::Arc;

use media_core::{AppDatabase, AvatarCache, ThumbnailCache};
use renamer::{PresetManager, RenameUndoManager};
use tokio::sync::Mutex;

use crate::config::{AppConfig, ConfigStore};
use crate::log_store::LogStore;
use crate::task_queue::TaskQueue;

pub struct AppState {
    pub db: Arc<AppDatabase>,
    pub config: Arc<Mutex<ConfigStore>>,
    pub tasks: Arc<TaskQueue>,
    pub thumbs: Arc<ThumbnailCache>,
    pub avatars: Arc<AvatarCache>,
    pub rename_undo: Arc<RenameUndoManager>,
    pub rename_presets: Arc<PresetManager>,
    pub logs: Arc<LogStore>,
    pub data_dir: PathBuf,
}

impl AppState {
    pub fn bootstrap(logs: Arc<LogStore>) -> anyhow::Result<Self> {
        let data_dir = app_data_dir()?;
        std::fs::create_dir_all(&data_dir)?;

        let db_path = data_dir.join("kaigua.sqlite3");
        let db = Arc::new(AppDatabase::open(&db_path)?);
        tracing::info!(path = %db_path.display(), "database opened");

        let config_path = data_dir.join("config.toml");
        let config = Arc::new(Mutex::new(ConfigStore::load_or_default(&config_path)?));

        let tasks = Arc::new(TaskQueue::new());
        let thumbs = Arc::new(ThumbnailCache::open_default()?);
        let avatars = Arc::new(AvatarCache::open_default()?);
        let rename_undo = Arc::new(RenameUndoManager::open(
            data_dir.join("rename_snapshots"),
        )?);
        let rename_presets = Arc::new(PresetManager::open(data_dir.join("rename_presets"))?);

        Ok(Self {
            db,
            config,
            tasks,
            thumbs,
            avatars,
            rename_undo,
            rename_presets,
            logs,
            data_dir,
        })
    }
}

fn app_data_dir() -> anyhow::Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| anyhow::anyhow!("no data directory"))?;
    Ok(base.join("kaigua"))
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppStatusDto {
    pub app_name: String,
    pub version: String,
    pub data_dir: String,
    pub database_path: String,
    pub library_count: i64,
    pub config: AppConfig,
    pub crates: CratesDto,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CratesDto {
    pub media_core: String,
    pub scraper_kit: String,
    pub renamer: String,
}
