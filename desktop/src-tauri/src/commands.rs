use std::sync::Arc;

use media_core::{Library, MediaItem, MediaMetaSummary, MediaMetadata, MediaType, TvEpisode, TvSeason};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::config::AppConfig;
use crate::state::{AppState, AppStatusDto, CratesDto};
use crate::task_queue::{TaskKind, TaskProgress, TaskSnapshot, TaskStatus};

#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> Result<AppStatusDto, String> {
    let library_count = state.db.library_count().map_err(err_string)?;
    let config = state.config.lock().await.config.clone();
    Ok(AppStatusDto {
        app_name: "kaigua".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: state.data_dir.display().to_string(),
        database_path: state.db.path().display().to_string(),
        library_count,
        config,
        crates: CratesDto {
            media_core: "media-core".into(),
            scraper_kit: scraper_kit::crate_name().into(),
            renamer: renamer::crate_name().into(),
        },
    })
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    Ok(state.config.lock().await.config.clone())
}

#[tauri::command]
pub async fn save_config(
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    let mut store = state.config.lock().await;
    store.config = config;
    store.save().map_err(err_string)?;
    Ok(store.config.clone())
}

#[tauri::command]
pub async fn list_libraries(state: State<'_, AppState>) -> Result<Vec<Library>, String> {
    state.db.list_libraries().map_err(err_string)
}

#[tauri::command]
pub async fn add_library(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    root_path: String,
    media_type: MediaType,
) -> Result<Library, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("library name is empty".into());
    }
    if root_path.trim().is_empty() {
        return Err("library path is empty".into());
    }
    let library = Library::new(name, root_path, media_type);
    state.db.insert_library(&library).map_err(err_string)?;
    let _ = enqueue_refresh_inner(&app, &state, library.id.clone()).await?;
    Ok(library)
}

#[tauri::command]
pub async fn rename_library(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<Library, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("library name is empty".into());
    }
    let mut library = state
        .db
        .get_library(&id)
        .map_err(err_string)?
        .ok_or_else(|| format!("library not found: {id}"))?;
    library.name = name;
    state.db.update_library(&library).map_err(err_string)?;
    Ok(library)
}

#[tauri::command]
pub async fn delete_library(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.db.delete_library(&id).map_err(err_string)
}

#[tauri::command]
pub async fn list_media_items(
    state: State<'_, AppState>,
    library_id: String,
) -> Result<Vec<MediaItem>, String> {
    state.db.list_media_items(&library_id).map_err(err_string)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaListPayload {
    pub items: Vec<MediaItem>,
    pub metadata: Vec<MediaMetaSummary>,
}

#[tauri::command]
pub async fn list_media_page(
    state: State<'_, AppState>,
    library_id: String,
) -> Result<MediaListPayload, String> {
    let items = state.db.list_media_items(&library_id).map_err(err_string)?;
    let metadata = state
        .db
        .list_metadata_summaries(&library_id)
        .map_err(err_string)?;
    Ok(MediaListPayload { items, metadata })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetailDto {
    pub item: MediaItem,
    pub metadata: Option<MediaMetadata>,
    pub seasons: Vec<TvSeason>,
    pub episodes: Vec<TvEpisode>,
}

#[tauri::command]
pub async fn get_media_detail(
    state: State<'_, AppState>,
    id: String,
) -> Result<MediaDetailDto, String> {
    let item = state
        .db
        .get_media_item(&id)
        .map_err(err_string)?
        .ok_or_else(|| format!("media item not found: {id}"))?;
    let metadata = state.db.fetch_metadata(&id).map_err(err_string)?;
    let seasons = state.db.fetch_seasons(&id).map_err(err_string)?;
    let mut episodes = Vec::new();
    for season in &seasons {
        episodes.extend(state.db.fetch_episodes(&season.id).map_err(err_string)?);
    }
    Ok(MediaDetailDto {
        item,
        metadata,
        seasons,
        episodes,
    })
}

#[tauri::command]
pub async fn resolve_poster_thumbnail(
    state: State<'_, AppState>,
    folder_path: String,
    poster_path: String,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<Option<String>, String> {
    let width = width.unwrap_or(100);
    let height = height.unwrap_or(148);
    let source = media_core::ThumbnailCache::join_poster_path(&folder_path, &poster_path);
    let thumbs = Arc::clone(&state.thumbs);
    let result = tokio::task::spawn_blocking(move || thumbs.ensure(&source, width, height))
        .await
        .map_err(|e| e.to_string())?;
    match result {
        Ok(path) => Ok(Some(path.display().to_string())),
        Err(media_core::ThumbnailError::Missing(_)) => Ok(None),
        Err(err) => Err(err.to_string()),
    }
}

#[tauri::command]
pub async fn refresh_library(
    app: AppHandle,
    state: State<'_, AppState>,
    library_id: String,
) -> Result<TaskSnapshot, String> {
    enqueue_refresh_inner(&app, &state, library_id).await
}

#[tauri::command]
pub async fn list_tasks(state: State<'_, AppState>) -> Result<Vec<TaskSnapshot>, String> {
    Ok(state.tasks.list().await)
}

#[tauri::command]
pub async fn enqueue_smoke_task(
    app: AppHandle,
    state: State<'_, AppState>,
    title: Option<String>,
) -> Result<TaskSnapshot, String> {
    let snapshot = state
        .tasks
        .enqueue_smoke(title.unwrap_or_else(|| "M0 smoke task".into()))
        .await;
    watch_task(app, Arc::clone(&state.tasks), snapshot.id.clone());
    Ok(snapshot)
}

#[tauri::command]
pub async fn cancel_task(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    Ok(state.tasks.cancel(&id).await)
}

async fn enqueue_refresh_inner(
    app: &AppHandle,
    state: &State<'_, AppState>,
    library_id: String,
) -> Result<TaskSnapshot, String> {
    let library = state
        .db
        .get_library(&library_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("library not found: {library_id}"))?;
    let excluded = state
        .config
        .lock()
        .await
        .config
        .scan_excluded_folders
        .clone();
    let db = Arc::clone(&state.db);
    let title = format!("Refresh · {}", library.name);

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Refresh, move |handle| {
            Box::pin(async move {
                let library_for_scan = library;
                let excluded_folders = excluded;
                let (progress_tx, mut progress_rx) =
                    tokio::sync::mpsc::unbounded_channel::<media_core::ScanProgress>();
                let scan = tokio::task::spawn_blocking(move || {
                    let mut last_emit = 0u32;
                    media_core::refresh_library(&db, &library_for_scan, &excluded_folders, |p| {
                        if p.discovered_count == 1
                            || p.discovered_count.saturating_sub(last_emit) >= 25
                        {
                            last_emit = p.discovered_count;
                            let _ = progress_tx.send(p);
                        }
                    })
                });

                while let Some(p) = progress_rx.recv().await {
                    if handle.is_cancelled() {
                        break;
                    }
                    handle
                        .update_progress(TaskProgress {
                            completed: p.discovered_count,
                            total: 0,
                            current: p.current_name,
                            stage_key: Some("scanFiles".into()),
                        })
                        .await;
                }

                let report = scan
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?;

                if handle.is_cancelled() {
                    return Err("cancelled".into());
                }
                handle
                    .update_progress(TaskProgress {
                        completed: report.new_item_count as u32,
                        total: report.new_item_count as u32,
                        current: format!("added {} items", report.new_item_count),
                        stage_key: Some("saveResults".into()),
                    })
                    .await;
                Ok(())
            })
        })
        .await;

    watch_task(app.clone(), Arc::clone(&state.tasks), snapshot.id.clone());
    let _ = app.emit("task-updated", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn scrape_library(
    app: AppHandle,
    state: State<'_, AppState>,
    library_id: String,
) -> Result<TaskSnapshot, String> {
    let library = state
        .db
        .get_library(&library_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("library not found: {library_id}"))?;
    let config = state.config.lock().await.config.clone();
    let db = Arc::clone(&state.db);
    let title = format!("Scrape All · {}", library.name);
    let options = scrape_options_from_config(&config);

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::BatchScrape, move |handle| {
            Box::pin(async move {
                let (progress_tx, mut progress_rx) =
                    tokio::sync::mpsc::unbounded_channel::<scraper_kit::ScrapeProgress>();
                let job = tokio::spawn(async move {
                    scraper_kit::scrape_library(db, &library_id, options, |p| {
                        let _ = progress_tx.send(p);
                    })
                    .await
                });
                while let Some(p) = progress_rx.recv().await {
                    if handle.is_cancelled() {
                        break;
                    }
                    handle
                        .update_progress(TaskProgress {
                            completed: p.completed,
                            total: p.total,
                            current: p.current,
                            stage_key: Some(p.stage_key),
                        })
                        .await;
                }
                job.await.map_err(|e| e.to_string())??;
                Ok(())
            })
        })
        .await;

    watch_task(app.clone(), Arc::clone(&state.tasks), snapshot.id.clone());
    let _ = app.emit("task-updated", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn scrape_items(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let config = state.config.lock().await.config.clone();
    let options = scrape_options_from_config(&config);
    let db = Arc::clone(&state.db);
    let title = format!("Scrape · {} items", item_ids.len());

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Scrape, move |handle| {
            Box::pin(async move {
                let total = item_ids.len() as u32;
                for (idx, id) in item_ids.into_iter().enumerate() {
                    if handle.is_cancelled() {
                        return Err("cancelled".into());
                    }
                    let item = db
                        .get_media_item(&id)
                        .map_err(err_string)?
                        .ok_or_else(|| format!("media item not found: {id}"))?;
                    handle
                        .update_progress(TaskProgress {
                            completed: idx as u32,
                            total,
                            current: item.title.clone(),
                            stage_key: Some("matching".into()),
                        })
                        .await;
                    scraper_kit::scrape_item(&db, &item, &options).await?;
                }
                Ok(())
            })
        })
        .await;

    watch_task(app.clone(), Arc::clone(&state.tasks), snapshot.id.clone());
    let _ = app.emit("task-updated", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn search_match_candidates(
    state: State<'_, AppState>,
    query: String,
    media_type: MediaType,
) -> Result<Vec<scraper_kit::SearchResult>, String> {
    let config = state.config.lock().await.config.clone();
    let coordinator = scraper_kit::ScraperCoordinator::new(scraper_keys(&config));
    coordinator
        .search_manual(&query, media_type, &config.metadata_language)
        .await
}

#[tauri::command]
pub async fn apply_manual_match(
    state: State<'_, AppState>,
    item_id: String,
    source_id: String,
) -> Result<(), String> {
    let config = state.config.lock().await.config.clone();
    let options = scrape_options_from_config(&config);
    let item = state
        .db
        .get_media_item(&item_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("media item not found: {item_id}"))?;
    scraper_kit::apply_manual_match(&state.db, &item, &source_id, &options).await
}

fn scrape_options_from_config(config: &AppConfig) -> scraper_kit::ScrapeOptions {
    scraper_kit::ScrapeOptions {
        language: config.metadata_language.clone(),
        concurrency: config.scrape_concurrency.max(1) as usize,
        keys: scraper_keys(config),
    }
}

fn scraper_keys(config: &AppConfig) -> scraper_kit::ScraperKeys {
    scraper_kit::ScraperKeys {
        tmdb: config.api_keys.tmdb.clone(),
        bangumi: config.api_keys.bangumi.clone(),
    }
}

fn watch_task(app: AppHandle, tasks: Arc<crate::task_queue::TaskQueue>, id: String) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            let list = tasks.list().await;
            if let Some(current) = list.into_iter().find(|t| t.id == id) {
                let _ = app.emit("task-updated", &current);
                if matches!(
                    current.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    let _ = app.emit("library-updated", ());
                    break;
                }
            } else {
                break;
            }
        }
    });
}

fn err_string(err: impl ToString) -> String {
    err.to_string()
}
