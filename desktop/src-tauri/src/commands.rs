use std::sync::Arc;

use media_core::{
    Library, MediaItem, MediaMetaSummary, MediaMetadata, MediaType, ScrapedStatus, ShowListStats,
    TvEpisode, TvSeason,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::config::AppConfig;
use crate::state::{AppState, AppStatusDto, CratesDto};
use crate::task_queue::{TaskKind, TaskProgress, TaskSnapshot, TaskStatus};

async fn ui_locale(state: &State<'_, AppState>) -> String {
    state.config.lock().await.config.ui_locale.clone()
}

fn loc_progress(locale: &str, name: &str) -> String {
    match name {
        "scan.checking" => crate::ui_i18n::t(locale, "prog.checking"),
        "scan.unchanged" => crate::ui_i18n::t(locale, "prog.unchanged"),
        _ => name.to_string(),
    }
}

fn loc_scrape_summary(locale: &str, raw: &str) -> String {
    if let Some((s, u, f)) = scraper_kit::ScrapeSummary::parse_result(raw) {
        return crate::ui_i18n::tf(
            locale,
            "prog.scrapeSummary",
            &[
                ("success", &s.to_string()),
                ("unmatched", &u.to_string()),
                ("failed", &f.to_string()),
            ],
        );
    }
    raw.to_string()
}

fn loc_err(locale: &str, err: String) -> String {
    if err.starts_with("err.") {
        crate::ui_i18n::t(locale, &err)
    } else {
        err
    }
}

fn auto_rename_after_scrape(
    db: &media_core::AppDatabase,
    ids: &[String],
    templates: &renamer::RenameTemplates,
    create_season_folders: bool,
) -> (u32, u32) {
    let mut ok = 0u32;
    let mut failed = 0u32;
    for id in ids {
        let Some(item) = db.get_media_item(id).ok().flatten() else {
            // May already have been merged into a canonical show.
            continue;
        };
        // Absorb season packs / release folders into the existing series first.
        if let Ok(true) = renamer::consolidate_show_item(db, &item, templates) {
            ok += 1;
            continue;
        }
        let Some(item) = db.get_media_item(id).ok().flatten() else {
            continue;
        };
        match renamer::rename_after_scrape_with_options(
            db,
            &item,
            templates,
            create_season_folders,
        ) {
            Ok(()) => ok += 1,
            Err(err) => {
                failed += 1;
                tracing::warn!(
                    item_id = %id,
                    title = %item.title,
                    error = %err,
                    "auto-rename after scrape failed"
                );
            }
        }
    }
    (ok, failed)
}

fn consolidate_after_scrape(
    db: &media_core::AppDatabase,
    ids: &[String],
    templates: &renamer::RenameTemplates,
) {
    for id in ids {
        let Some(item) = db.get_media_item(id).ok().flatten() else {
            continue;
        };
        if let Err(err) = renamer::consolidate_show_item(db, &item, templates) {
            tracing::warn!(
                item_id = %id,
                title = %item.title,
                error = %err,
                "consolidate duplicate show failed"
            );
        }
    }
}

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
    app: AppHandle,
    state: State<'_, AppState>,
    config: AppConfig,
) -> Result<AppConfig, String> {
    let tray_enabled = config.tray_enabled;
    let mut store = state.config.lock().await;
    store.config = config;
    store.save().map_err(err_string)?;
    let saved = store.config.clone();
    drop(store);
    crate::tray::set_enabled(&app, tray_enabled);
    crate::tray::set_locale(&app, &saved.ui_locale);
    Ok(saved)
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
pub async fn path_is_dir(path: String) -> Result<bool, String> {
    Ok(std::path::Path::new(path.trim()).is_dir())
}

/// LIB-08: rebind library root when the previous path is stale / moved.
#[tauri::command]
pub async fn rebind_library(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    root_path: String,
) -> Result<Library, String> {
    let root_path = root_path.trim().to_string();
    if root_path.is_empty() {
        return Err("library path is empty".into());
    }
    if !std::path::Path::new(&root_path).is_dir() {
        return Err("selected path is not a directory".into());
    }
    let mut library = state
        .db
        .get_library(&id)
        .map_err(err_string)?
        .ok_or_else(|| format!("library not found: {id}"))?;
    library.root_path = root_path;
    library.bookmark_data = None;
    state.db.update_library(&library).map_err(err_string)?;
    // Path changed → wipe scan state so next refresh re-bootstraps.
    let _ = state.db.clear_scan_states(&library.id);
    let _ = enqueue_refresh_inner(&app, &state, library.id.clone()).await?;
    let _ = app.emit("library-updated", ());
    Ok(library)
}

#[tauri::command]
pub async fn clear_thumbnail_cache(state: State<'_, AppState>) -> Result<usize, String> {
    let thumbs = state.thumbs.clear_all().map_err(err_string)?;
    let avatars = state.avatars.clear().map_err(err_string)?;
    Ok(thumbs + avatars)
}

#[tauri::command]
pub async fn resolve_actor_avatar(
    state: State<'_, AppState>,
    url: String,
) -> Result<Option<String>, String> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Ok(None);
    }
    if let Some(cached) = state.avatars.cached_path(&url) {
        return Ok(Some(cached.display().to_string()));
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(err_string)?;
    let response = client.get(&url).send().await.map_err(err_string)?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let bytes = response.bytes().await.map_err(err_string)?;
    let avatars = Arc::clone(&state.avatars);
    let stored = tokio::task::spawn_blocking(move || avatars.store(&url, &bytes))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err_string)?;
    Ok(Some(stored.display().to_string()))
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
    pub show_stats: Vec<ShowListStats>,
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
    let show_stats = state.db.list_show_stats(&library_id).map_err(err_string)?;
    Ok(MediaListPayload {
        items,
        metadata,
        show_stats,
    })
}

/// Merge duplicate TV/anime shows in the background (same TMDB / title+year).
/// Cheap no-op when there is nothing to merge; never call from the list hot path.
#[tauri::command]
pub async fn consolidate_library_shows(
    state: State<'_, AppState>,
    library_id: String,
) -> Result<u32, String> {
    let library = state
        .db
        .get_library(&library_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("library not found: {library_id}"))?;
    if !matches!(
        library.media_type,
        MediaType::TvShow | MediaType::Anime
    ) {
        return Ok(0);
    }
    let templates = state.config.lock().await.config.rename_templates();
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || {
        renamer::consolidate_library_duplicate_shows(&db, &library_id, &templates).unwrap_or(0)
    })
    .await
    .map_err(|e| e.to_string())
    .map(|n| n as u32)
}

/// Absorb selected TV/anime items into a better canonical duplicate (same TMDB / title+year).
#[tauri::command]
pub async fn consolidate_media_items(
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<u32, String> {
    if item_ids.is_empty() {
        return Ok(0);
    }
    let templates = state.config.lock().await.config.rename_templates();
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || {
        let mut merged = 0u32;
        for id in item_ids {
            let Some(item) = db.get_media_item(&id).ok().flatten() else {
                continue;
            };
            if !matches!(item.media_type, MediaType::TvShow | MediaType::Anime) {
                continue;
            }
            if renamer::consolidate_show_item(&db, &item, &templates).unwrap_or(false) {
                merged += 1;
            }
        }
        merged
    })
    .await
    .map_err(|e| e.to_string())
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
    let (seasons, episodes) = if matches!(
        item.media_type,
        MediaType::TvShow | MediaType::Anime
    ) {
        let seasons = state.db.fetch_seasons(&id).map_err(err_string)?;
        let mut episodes = Vec::new();
        for season in &seasons {
            episodes.extend(state.db.fetch_episodes(&season.id).map_err(err_string)?);
        }
        (seasons, episodes)
    } else {
        (Vec::new(), Vec::new())
    };
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
    allow_fallbacks: Option<bool>,
) -> Result<Option<String>, String> {
    let width = width.unwrap_or(media_core::POSTER_THUMB_WIDTH);
    let height = height.unwrap_or(media_core::POSTER_THUMB_HEIGHT);
    let allow_fallbacks = allow_fallbacks.unwrap_or(true);
    let Some(source) = media_core::ThumbnailCache::resolve_poster_source_with_fallbacks(
        &folder_path,
        &poster_path,
        allow_fallbacks,
    ) else {
        return Ok(None);
    };
    let thumbs = Arc::clone(&state.thumbs);
    let result = tokio::task::spawn_blocking(move || thumbs.ensure(&source, width, height))
        .await
        .map_err(|e| e.to_string())?;
    match result {
        // Return cache file path; frontend uses convertFileSrc (faster than base64 IPC).
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
pub async fn refresh_media_items(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let locale = ui_locale(&state).await;
    let title = if item_ids.len() == 1 {
        crate::ui_i18n::t(&locale, "task.refreshItems")
    } else {
        crate::ui_i18n::tf(&locale, "task.refreshItemsN", &[("n", &item_ids.len().to_string())])
    };
    let excluded = state.config.lock().await.config.scan_excluded_folders.clone();
    let db = Arc::clone(&state.db);
    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Refresh, item_ids.first().cloned(), move |handle| {
            let locale = locale.clone();
            Box::pin(async move {
                let total = item_ids.len() as u32;
                handle
                    .update_progress(TaskProgress {
                        completed: 0,
                        total,
                        current: crate::ui_i18n::t(&locale, "prog.refreshing"),
                        stage_key: Some("refreshItems".into()),
                    })
                    .await;
                let report = tokio::task::spawn_blocking(move || {
                    media_core::refresh_items(&db, &item_ids, &excluded)
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(err_string)?;
                if handle.is_cancelled() {
                    return Err("cancelled".into());
                }
                handle
                    .update_progress(TaskProgress {
                        completed: total,
                        total,
                        current: crate::ui_i18n::tf(
                            &locale,
                            "prog.refreshDone",
                            &[
                                ("ok", &report.refreshed.to_string()),
                                ("removed", &report.removed.to_string()),
                            ],
                        ),
                        stage_key: Some("refreshItems".into()),
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
    if let Some(existing) = state
        .tasks
        .find_active(TaskKind::Refresh, &library_id)
        .await
    {
        let _ = app.emit("task-updated", &existing);
        return Ok(existing);
    }

    let library = state
        .db
        .get_library(&library_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("library not found: {library_id}"))?;
    let (excluded, locale, rename_templates, media_type) = {
        let cfg = state.config.lock().await;
        (
            cfg.config.scan_excluded_folders.clone(),
            cfg.config.ui_locale.clone(),
            cfg.config.rename_templates(),
            library.media_type,
        )
    };
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(&locale, "task.refreshLib", &[("name", &library.name)]);
    let target_id = Some(library_id.clone());
    let library_id_for_merge = library_id;

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Refresh, target_id, move |handle| {
            let locale = locale.clone();
            Box::pin(async move {
                let library_for_scan = library;
                let excluded_folders = excluded;
                let db_scan = Arc::clone(&db);
                let (progress_tx, mut progress_rx) =
                    tokio::sync::mpsc::unbounded_channel::<media_core::ScanProgress>();
                let scan = tokio::task::spawn_blocking(move || {
                    let mut last_emit = 0u32;
                    let mut saw_check = false;
                    media_core::refresh_library(&db_scan, &library_for_scan, &excluded_folders, |p| {
                        let is_check = p.discovered_count == 0
                            && (p.current_name == "scan.checking"
                                || p.current_name == "scan.unchanged"
                                || p.current_name.starts_with("检查")
                                || p.current_name.starts_with("目录")
                                || p.current_name.starts_with("scan."));
                        if is_check {
                            if !saw_check || p.current_name == "scan.unchanged" || p.current_name.starts_with("目录无变更") {
                                saw_check = true;
                                let _ = progress_tx.send(p);
                            }
                            return;
                        }
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
                    let stage_key = if p.discovered_count == 0
                        && (p.current_name == "scan.checking"
                            || p.current_name == "scan.unchanged"
                            || p.current_name.starts_with("检查")
                            || p.current_name.starts_with("目录")
                            || p.current_name.starts_with("scan."))
                    {
                        "checkDirectories"
                    } else {
                        "scanFiles"
                    };
                    handle
                        .update_progress(TaskProgress {
                            completed: p.discovered_count,
                            total: 0,
                            current: loc_progress(&locale, &p.current_name),
                            stage_key: Some(stage_key.into()),
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

                // Same TMDB season packs → merge into the canonical show (also on early-exit).
                let mut merged_n = 0usize;
                if matches!(
                    media_type,
                    media_core::MediaType::TvShow | media_core::MediaType::Anime
                ) {
                    let db_merge = Arc::clone(&db);
                    let templates = rename_templates.clone();
                    let lib_id = library_id_for_merge.clone();
                    merged_n = tokio::task::spawn_blocking(move || {
                        renamer::consolidate_library_duplicate_shows(&db_merge, &lib_id, &templates)
                    })
                    .await
                    .map_err(|e| e.to_string())?
                    .unwrap_or(0);
                }

                if merged_n > 0 {
                    handle
                        .update_progress(TaskProgress {
                            completed: merged_n as u32,
                            total: merged_n as u32,
                            current: crate::ui_i18n::tf(
                                &locale,
                                "prog.mergedShows",
                                &[("n", &merged_n.to_string())],
                            ),
                            stage_key: Some("saveResults".into()),
                        })
                        .await;
                } else if report.early_exit {
                    handle
                        .update_progress(TaskProgress {
                            completed: 0,
                            total: 0,
                            current: crate::ui_i18n::t(&locale, "prog.unchanged"),
                            stage_key: Some("checkDirectories".into()),
                        })
                        .await;
                } else {
                    handle
                        .update_progress(TaskProgress {
                            completed: report.new_item_count as u32,
                            total: report.new_item_count as u32,
                            current: crate::ui_i18n::tf(&locale, "prog.added", &[("n", &report.new_item_count.to_string())]),
                            stage_key: Some("saveResults".into()),
                        })
                        .await;
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
    let locale = config.ui_locale.clone();
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(&locale, "task.scrapeAll", &[("name", &library.name)]);
    let options = scrape_options_from_config(&config);
    let target_id = Some(library_id.clone());

    if let Some(existing) = state
        .tasks
        .find_active(TaskKind::BatchScrape, &library_id)
        .await
    {
        let _ = app.emit("task-updated", &existing);
        return Ok(existing);
    }

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::BatchScrape, target_id, move |handle| {
            Box::pin(async move {
                let (progress_tx, mut progress_rx) =
                    tokio::sync::mpsc::unbounded_channel::<scraper_kit::ScrapeProgress>();
                let db_job = Arc::clone(&db);
                let job = tokio::spawn(async move {
                    scraper_kit::scrape_library(db_job, &library_id, options, |p| {
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
                let summary = job
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| loc_err(&locale, e))?;
                let success_ids = summary.success_ids.clone();
                handle
                    .update_progress(TaskProgress {
                        completed: summary.success_ids.len() as u32
                            + summary.unmatched
                            + summary.failed,
                        total: summary.success_ids.len() as u32
                            + summary.unmatched
                            + summary.failed,
                        current: loc_scrape_summary(&locale, &summary.format_result()),
                        stage_key: Some("saveResults".into()),
                    })
                    .await;
                if !success_ids.is_empty() {
                    let templates = config.rename_templates();
                    if config.rename_auto_after_scrape {
                        handle
                            .update_progress(TaskProgress {
                                completed: success_ids.len() as u32,
                                total: success_ids.len() as u32,
                                current: crate::ui_i18n::t(&locale, "prog.autoRename"),
                                stage_key: Some("rename".into()),
                            })
                            .await;
                        let (renamed, rename_failed) = auto_rename_after_scrape(
                            &db,
                            &success_ids,
                            &templates,
                            config.rename_create_season_folders,
                        );
                        let mut summary_text = loc_scrape_summary(&locale, &summary.format_result());
                        if rename_failed > 0 {
                            summary_text = format!(
                                "{summary_text} · {}",
                                crate::ui_i18n::tf(
                                    &locale,
                                    "prog.autoRenameResult",
                                    &[
                                        ("ok", &renamed.to_string()),
                                        ("failed", &rename_failed.to_string()),
                                    ],
                                )
                            );
                        }
                        handle
                            .update_progress(TaskProgress {
                                completed: success_ids.len() as u32
                                    + summary.unmatched
                                    + summary.failed,
                                total: success_ids.len() as u32
                                    + summary.unmatched
                                    + summary.failed,
                                current: summary_text,
                                stage_key: Some("saveResults".into()),
                            })
                            .await;
                    } else {
                        consolidate_after_scrape(&db, &success_ids, &templates);
                    }
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
pub async fn scrape_items(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let config = state.config.lock().await.config.clone();
    let locale = config.ui_locale.clone();
    let options = scrape_options_from_config(&config);
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(&locale, "task.scrapeN", &[("n", &item_ids.len().to_string())]);

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Scrape, item_ids.first().cloned(), move |handle| {
            Box::pin(async move {
                let mut summary = scraper_kit::ScrapeSummary::default();
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
                    match scraper_kit::scrape_item(&db, &item, &options)
                        .await
                        .map_err(|e| loc_err(&locale, e))?
                    {
                        scraper_kit::ScrapeItemOutcome::Matched => {
                            summary.success_ids.push(id);
                        }
                        scraper_kit::ScrapeItemOutcome::Unmatched => {
                            summary.unmatched += 1;
                        }
                        scraper_kit::ScrapeItemOutcome::Failed => {
                            summary.failed += 1;
                        }
                    }
                }
                handle
                    .update_progress(TaskProgress {
                        completed: total,
                        total,
                        current: loc_scrape_summary(&locale, &summary.format_result()),
                        stage_key: Some("saveResults".into()),
                    })
                    .await;
                if !summary.success_ids.is_empty() {
                    let templates = config.rename_templates();
                    if config.rename_auto_after_scrape {
                        handle
                            .update_progress(TaskProgress {
                                completed: total,
                                total,
                                current: crate::ui_i18n::t(&locale, "prog.autoRename"),
                                stage_key: Some("rename".into()),
                            })
                            .await;
                        let (renamed, rename_failed) = auto_rename_after_scrape(
                            &db,
                            &summary.success_ids,
                            &templates,
                            config.rename_create_season_folders,
                        );
                        let mut summary_text = loc_scrape_summary(&locale, &summary.format_result());
                        if rename_failed > 0 {
                            summary_text = format!(
                                "{summary_text} · {}",
                                crate::ui_i18n::tf(
                                    &locale,
                                    "prog.autoRenameResult",
                                    &[
                                        ("ok", &renamed.to_string()),
                                        ("failed", &rename_failed.to_string()),
                                    ],
                                )
                            );
                        }
                        handle
                            .update_progress(TaskProgress {
                                completed: total,
                                total,
                                current: summary_text,
                                stage_key: Some("saveResults".into()),
                            })
                            .await;
                    } else {
                        consolidate_after_scrape(&db, &summary.success_ids, &templates);
                    }
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
pub async fn rescrape_items(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let mut scraped_ids = Vec::new();
    for id in &item_ids {
        let item = state
            .db
            .get_media_item(id)
            .map_err(err_string)?
            .ok_or_else(|| format!("media item not found: {id}"))?;
        if item.status == ScrapedStatus::Scraped {
            scraped_ids.push(id.clone());
        }
    }
    if scraped_ids.is_empty() {
        return Err("no scraped items selected".into());
    }

    let config = state.config.lock().await.config.clone();
    let locale = config.ui_locale.clone();
    let options = scrape_options_from_config(&config);
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(&locale, "task.rescrapeN", &[("n", &scraped_ids.len().to_string())]);

    let snapshot = state
        .tasks
        .enqueue(
            title,
            TaskKind::Rescrape,
            scraped_ids.first().cloned(),
            move |handle| {
                Box::pin(async move {
                    let mut summary = scraper_kit::ScrapeSummary::default();
                    let total = scraped_ids.len() as u32;
                    for (idx, id) in scraped_ids.into_iter().enumerate() {
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
                        match scraper_kit::scrape_item(&db, &item, &options)
                            .await
                            .map_err(|e| loc_err(&locale, e))?
                        {
                            scraper_kit::ScrapeItemOutcome::Matched => {
                                summary.success_ids.push(id);
                            }
                            scraper_kit::ScrapeItemOutcome::Unmatched => {
                                summary.unmatched += 1;
                            }
                            scraper_kit::ScrapeItemOutcome::Failed => {
                                summary.failed += 1;
                            }
                        }
                    }
                    handle
                        .update_progress(TaskProgress {
                            completed: total,
                            total,
                            current: loc_scrape_summary(&locale, &summary.format_result()),
                            stage_key: Some("saveResults".into()),
                        })
                        .await;
                    if !summary.success_ids.is_empty() {
                        let templates = config.rename_templates();
                        if config.rename_auto_after_scrape {
                            handle
                                .update_progress(TaskProgress {
                                    completed: total,
                                    total,
                                    current: crate::ui_i18n::t(&locale, "prog.autoRename"),
                                    stage_key: Some("rename".into()),
                                })
                                .await;
                            let (renamed, rename_failed) = auto_rename_after_scrape(
                                &db,
                                &summary.success_ids,
                                &templates,
                                config.rename_create_season_folders,
                            );
                            let mut summary_text =
                                loc_scrape_summary(&locale, &summary.format_result());
                            if rename_failed > 0 {
                                summary_text = format!(
                                    "{summary_text} · {}",
                                    crate::ui_i18n::tf(
                                        &locale,
                                        "prog.autoRenameResult",
                                        &[
                                            ("ok", &renamed.to_string()),
                                            ("failed", &rename_failed.to_string()),
                                        ],
                                    )
                                );
                            }
                            handle
                                .update_progress(TaskProgress {
                                    completed: total,
                                    total,
                                    current: summary_text,
                                    stage_key: Some("saveResults".into()),
                                })
                                .await;
                        } else {
                            consolidate_after_scrape(&db, &summary.success_ids, &templates);
                        }
                    }
                    Ok(())
                })
            },
        )
        .await;

    watch_task(app.clone(), Arc::clone(&state.tasks), snapshot.id.clone());
    let _ = app.emit("task-updated", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn scrape_season(
    state: State<'_, AppState>,
    media_item_id: String,
    season_number: i32,
) -> Result<(), String> {
    let config = state.config.lock().await.config.clone();
    let locale = config.ui_locale.clone();
    let options = scrape_options_from_config(&config);
    let item = state
        .db
        .get_media_item(&media_item_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("media item not found: {media_item_id}"))?;
    scraper_kit::scrape_season(&state.db, &item, season_number, &options)
        .await
        .map_err(|e| loc_err(&locale, e))
}

#[tauri::command]
pub async fn apply_rename_templates(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let config = state.config.lock().await.config.clone();
    let locale = config.ui_locale.clone();
    let templates = config.rename_templates();
    let create_season_folders = config.rename_create_season_folders;
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(&locale, "task.renameN", &[("n", &item_ids.len().to_string())]);

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Rename, item_ids.first().cloned(), move |handle| {
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
                            stage_key: Some("rename".into()),
                        })
                        .await;
                    // Season packs that share TMDB with an existing show are absorbed first.
                    let _ = renamer::consolidate_show_item(&db, &item, &templates);
                    let Some(item) = db
                        .get_media_item(&id)
                        .map_err(err_string)?
                    else {
                        continue;
                    };
                    renamer::rename_after_scrape_with_options(
                        &db,
                        &item,
                        &templates,
                        create_season_folders,
                    )
                    .map_err(|e| e.to_string())?;
                }
                handle
                    .update_progress(TaskProgress {
                        completed: total,
                        total,
                        current: crate::ui_i18n::tf(&locale, "prog.renamed", &[("n", &total.to_string())]),
                        stage_key: Some("rename".into()),
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
pub async fn organize_season_folders(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let mut targets = Vec::new();
    for id in &item_ids {
        let item = state
            .db
            .get_media_item(id)
            .map_err(err_string)?
            .ok_or_else(|| format!("media item not found: {id}"))?;
        if item.status == ScrapedStatus::Scraped
            && matches!(item.media_type, MediaType::TvShow | MediaType::Anime)
        {
            targets.push(id.clone());
        }
    }
    if targets.is_empty() {
        return Err("no scraped tv/anime items selected".into());
    }

    let (templates, locale) = {
        let cfg = state.config.lock().await;
        (cfg.config.rename_templates(), cfg.config.ui_locale.clone())
    };
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(&locale, "task.organizeN", &[("n", &targets.len().to_string())]);

    let snapshot = state
        .tasks
        .enqueue(
            title,
            TaskKind::Organize,
            targets.first().cloned(),
            move |handle| {
                Box::pin(async move {
                    let total = targets.len() as u32;
                    for (idx, id) in targets.into_iter().enumerate() {
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
                                stage_key: Some("organize".into()),
                            })
                            .await;
                        renamer::organize_season_folders(&db, &item, &templates)
                            .map_err(|e| e.to_string())?;
                    }
                    handle
                        .update_progress(TaskProgress {
                            completed: total,
                            total,
                            current: crate::ui_i18n::tf(&locale, "prog.organized", &[("n", &total.to_string())]),
                            stage_key: Some("organize".into()),
                        })
                        .await;
                    Ok(())
                })
            },
        )
        .await;

    watch_task(app.clone(), Arc::clone(&state.tasks), snapshot.id.clone());
    let _ = app.emit("task-updated", &snapshot);
    Ok(snapshot)
}

#[tauri::command]
pub async fn scan_media_residuals(
    state: State<'_, AppState>,
    item_ids: Vec<String>,
) -> Result<Vec<media_core::ResidualCandidate>, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }
    let db = Arc::clone(&state.db);
    tokio::task::spawn_blocking(move || media_core::find_residuals(&db, &item_ids))
        .await
        .map_err(|e| e.to_string())?
        .map_err(err_string)
}

#[tauri::command]
pub async fn cleanup_media_residuals(
    app: AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
) -> Result<TaskSnapshot, String> {
    if paths.is_empty() {
        return Err("no residual files selected".into());
    }
    let locale = ui_locale(&state).await;
    let title = crate::ui_i18n::tf(&locale, "task.cleanupN", &[("n", &paths.len().to_string())]);
    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::Cleanup, None, move |handle| {
            let locale = locale.clone();
            Box::pin(async move {
                let total = paths.len() as u32;
                handle
                    .update_progress(TaskProgress {
                        completed: 0,
                        total,
                        current: crate::ui_i18n::t(&locale, "prog.cleaning"),
                        stage_key: Some("cleanup".into()),
                    })
                    .await;
                let removed = tokio::task::spawn_blocking(move || {
                    media_core::perform_cleanup(&paths)
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(err_string)?;
                if handle.is_cancelled() {
                    return Err("cancelled".into());
                }
                handle
                    .update_progress(TaskProgress {
                        completed: removed as u32,
                        total,
                        current: crate::ui_i18n::tf(&locale, "prog.cleaned", &[("n", &removed.to_string())]),
                        stage_key: Some("cleanup".into()),
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
pub async fn delete_media_items(
    app: AppHandle,
    state: State<'_, AppState>,
    item_ids: Vec<String>,
    also_trash: bool,
) -> Result<usize, String> {
    if item_ids.is_empty() {
        return Err("no items selected".into());
    }

    let mut folders_to_remove = Vec::new();
    if also_trash {
        for id in &item_ids {
            if let Some(item) = state.db.get_media_item(id).map_err(err_string)? {
                if !item.folder_path.is_empty() {
                    folders_to_remove.push(item.folder_path);
                }
            }
        }
    }

    let deleted = state.db.delete_media_items(&item_ids).map_err(err_string)?;

    if also_trash {
        let fs = media_core::FilesystemService::new();
        for folder in folders_to_remove {
            let path = std::path::PathBuf::from(&folder);
            if !path.exists() {
                continue;
            }
            // PLAT-04: trash not available yet — hard delete after UI confirmation.
            match fs.trash_item(&path) {
                Ok(_) => {}
                Err(media_core::FilesystemError::TrashUnavailable) => {
                    fs.remove_item(&path)
                        .map_err(|e| format!("delete files failed ({folder}): {e}"))?;
                }
                Err(e) => return Err(e.to_string()),
            }
        }
    }

    let _ = app.emit("library-updated", ());
    Ok(deleted)
}

#[tauri::command]
pub async fn search_match_candidates(
    state: State<'_, AppState>,
    query: String,
    media_type: MediaType,
) -> Result<Vec<scraper_kit::SearchResult>, String> {
    let config = state.config.lock().await.config.clone();
    let locale = config.ui_locale.clone();
    let coordinator = scraper_kit::ScraperCoordinator::new(scraper_keys(&config));
    coordinator
        .search_manual(&query, media_type, &config.metadata_language)
        .await
        .map_err(|e| loc_err(&locale, e))
}

#[tauri::command]
pub async fn apply_manual_match(
    app: AppHandle,
    state: State<'_, AppState>,
    item_id: String,
    source_id: String,
) -> Result<TaskSnapshot, String> {
    let config = state.config.lock().await.config.clone();
    let locale = config.ui_locale.clone();
    let options = scrape_options_from_config(&config);
    let item = state
        .db
        .get_media_item(&item_id)
        .map_err(err_string)?
        .ok_or_else(|| format!("media item not found: {item_id}"))?;
    let db = Arc::clone(&state.db);
    let title = crate::ui_i18n::tf(
        &locale,
        "task.manualMatch",
        &[("title", &item.title)],
    );
    let target_id = Some(item_id.clone());

    let snapshot = state
        .tasks
        .enqueue(title, TaskKind::ManualMatch, target_id, move |handle| {
            let locale = locale.clone();
            Box::pin(async move {
                handle
                    .update_progress(TaskProgress {
                        completed: 0,
                        total: 1,
                        current: item.title.clone(),
                        stage_key: Some("matching".into()),
                    })
                    .await;
                scraper_kit::apply_manual_match(&db, &item, &source_id, &options)
                    .await
                    .map_err(|e| loc_err(&locale, e))?;
                if handle.is_cancelled() {
                    return Err("cancelled".into());
                }
                let templates = config.rename_templates();
                if config.rename_auto_after_scrape {
                    handle
                        .update_progress(TaskProgress {
                            completed: 0,
                            total: 1,
                            current: crate::ui_i18n::t(&locale, "prog.autoRename"),
                            stage_key: Some("rename".into()),
                        })
                        .await;
                    let _ = auto_rename_after_scrape(
                        &db,
                        &[item.id.clone()],
                        &templates,
                        config.rename_create_season_folders,
                    );
                } else {
                    consolidate_after_scrape(&db, &[item.id.clone()], &templates);
                }
                handle
                    .update_progress(TaskProgress {
                        completed: 1,
                        total: 1,
                        current: item.title.clone(),
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

fn scrape_options_from_config(config: &AppConfig) -> scraper_kit::ScrapeOptions {
    scraper_kit::ScrapeOptions {
        language: config.metadata_language.clone(),
        concurrency: config.scrape_concurrency.max(1) as usize,
        keys: scraper_keys(config),
        nfo_format: config.nfo_format.clone(),
    }
}

fn scraper_keys(config: &AppConfig) -> scraper_kit::ScraperKeys {
    scraper_kit::ScraperKeys {
        tmdb: config.api_keys.tmdb.clone(),
        bangumi: config.api_keys.bangumi.clone(),
        omdb: config.api_keys.omdb.clone(),
        tvdb: config.api_keys.tvdb.clone(),
    }
}

fn watch_task(app: AppHandle, tasks: Arc<crate::task_queue::TaskQueue>, id: String) {
    tauri::async_runtime::spawn(async move {
        let mut last_fingerprint = String::new();
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            let list = tasks.list().await;
            if let Some(current) = list.into_iter().find(|t| t.id == id) {
                let fingerprint = task_fingerprint(&current);
                if fingerprint != last_fingerprint {
                    last_fingerprint = fingerprint;
                    let _ = app.emit("task-updated", &current);
                }
                if matches!(
                    current.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    let _ = app.emit("library-updated", ());
                    if current.status == TaskStatus::Completed {
                        if let Some(state) = app.try_state::<AppState>() {
                            schedule_thumb_warm(
                                Arc::clone(&state.db),
                                Arc::clone(&state.thumbs),
                                current.kind,
                                current.target_id.clone(),
                            );
                        }
                    }
                    break;
                }
            } else {
                break;
            }
        }
    });
}

fn schedule_thumb_warm(
    db: Arc<media_core::AppDatabase>,
    thumbs: Arc<media_core::ThumbnailCache>,
    kind: TaskKind,
    target_id: Option<String>,
) {
    let Some(target_id) = target_id else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let warm = tokio::task::spawn_blocking(move || match kind {
            TaskKind::Refresh | TaskKind::BatchScrape => {
                warm_library_posters(&db, &thumbs, &target_id)
            }
            TaskKind::Scrape | TaskKind::ManualMatch => {
                warm_item_poster(&db, &thumbs, &target_id)
            }
            _ => 0,
        })
        .await;
        match warm {
            Ok(n) if n > 0 => tracing::info!(count = n, "poster thumbs warmed"),
            Ok(_) => {}
            Err(err) => tracing::warn!(error = %err, "poster warm task join failed"),
        }
    });
}

fn warm_library_posters(
    db: &media_core::AppDatabase,
    thumbs: &media_core::ThumbnailCache,
    library_id: &str,
) -> usize {
    let Ok(items) = db.list_media_items(library_id) else {
        return 0;
    };
    let Ok(metas) = db.list_metadata_summaries(library_id) else {
        return 0;
    };
    let mut by_id = std::collections::HashMap::new();
    for item in &items {
        by_id.insert(item.id.clone(), item);
    }
    let mut jobs = Vec::new();
    for meta in metas {
        let Some(poster) = meta.poster_path.as_deref().filter(|p| !p.is_empty()) else {
            continue;
        };
        let Some(item) = by_id.get(&meta.media_item_id) else {
            continue;
        };
        let Some(source) =
            media_core::ThumbnailCache::resolve_poster_source(&item.folder_path, poster)
        else {
            continue;
        };
        jobs.push(source);
    }
    if jobs.is_empty() {
        return 0;
    }

    use std::sync::atomic::{AtomicUsize, Ordering};
    let warmed = AtomicUsize::new(0);
    let workers = 4usize.min(jobs.len());
    let chunk = (jobs.len() + workers - 1) / workers;
    std::thread::scope(|scope| {
        for piece in jobs.chunks(chunk.max(1)) {
            let piece = piece.to_vec();
            let warmed = &warmed;
            scope.spawn(move || {
                for source in &piece {
                    if thumbs.ensure_poster_thumb(source).is_ok() {
                        warmed.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
        }
    });
    warmed.load(Ordering::Relaxed)
}

fn warm_item_poster(
    db: &media_core::AppDatabase,
    thumbs: &media_core::ThumbnailCache,
    item_id: &str,
) -> usize {
    let Ok(Some(item)) = db.get_media_item(item_id) else {
        return 0;
    };
    let Ok(Some(meta)) = db.fetch_metadata(item_id) else {
        return 0;
    };
    let Some(poster) = meta.poster_path.as_deref().filter(|p| !p.is_empty()) else {
        return 0;
    };
    let Some(source) =
        media_core::ThumbnailCache::resolve_poster_source(&item.folder_path, poster)
    else {
        return 0;
    };
    if thumbs.ensure_poster_thumb(&source).is_ok() {
        1
    } else {
        0
    }
}

fn task_fingerprint(task: &TaskSnapshot) -> String {
    let progress = task.progress.as_ref().map(|p| {
        format!(
            "{}:{}:{}:{}",
            p.completed,
            p.total,
            p.current,
            p.stage_key.as_deref().unwrap_or("")
        )
    });
    format!(
        "{:?}|{:?}|{}",
        task.status,
        progress,
        task.error_message.as_deref().unwrap_or("")
    )
}

#[tauri::command]
pub async fn open_renamer_window(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let locale = ui_locale(&state).await;
    if let Some(existing) = app.get_webview_window("renamer") {
        let _ = existing.show();
        let _ = existing.set_focus();
        return Ok(());
    }
    let builder = tauri::WebviewWindowBuilder::new(
        &app,
        "renamer",
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title(crate::ui_i18n::t(&locale, "window.renamer"))
    .inner_size(1040.0, 740.0)
    .min_inner_size(800.0, 560.0);

    // Match main-window immersive chrome on Windows (macOS keeps system decorations).
    #[cfg(target_os = "windows")]
    let builder = builder.decorations(false);

    builder.build().map_err(err_string)?;
    Ok(())
}

#[tauri::command]
pub async fn renamer_collect_files(paths: Vec<String>) -> Result<Vec<renamer::FileEntry>, String> {
    let mut out = Vec::new();
    for raw in paths {
        let path = std::path::PathBuf::from(&raw);
        collect_paths_into(&path, &mut out).map_err(err_string)?;
        if out.len() > MAX_RENAMER_FILES {
            return Err(format!("too many files (max {MAX_RENAMER_FILES})"));
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn renamer_preview(
    files: Vec<renamer::FileEntry>,
    pipeline: renamer::RulePipeline,
) -> Result<Vec<renamer::PreviewResult>, String> {
    Ok(renamer::preview(&files, &pipeline))
}

#[tauri::command]
pub async fn renamer_execute(
    state: State<'_, AppState>,
    previews: Vec<renamer::PreviewResult>,
) -> Result<Vec<renamer::CompletedRename>, String> {
    renamer::execute(&previews, &state.rename_undo).map_err(err_string)
}

#[tauri::command]
pub async fn renamer_undo_last(state: State<'_, AppState>) -> Result<usize, String> {
    state.rename_undo.undo_last().map_err(err_string)
}

#[tauri::command]
pub async fn renamer_snapshot_count(state: State<'_, AppState>) -> Result<usize, String> {
    Ok(state.rename_undo.snapshots().map_err(err_string)?.len())
}

#[tauri::command]
pub async fn renamer_list_presets(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.rename_presets.list_presets().map_err(err_string)
}

#[tauri::command]
pub async fn renamer_save_preset(
    state: State<'_, AppState>,
    name: String,
    pipeline: renamer::RulePipeline,
) -> Result<(), String> {
    state
        .rename_presets
        .save(&name, &pipeline)
        .map_err(err_string)
}

#[tauri::command]
pub async fn renamer_load_preset(
    state: State<'_, AppState>,
    name: String,
) -> Result<Option<renamer::RulePipeline>, String> {
    state.rename_presets.load(&name).map_err(err_string)
}

#[tauri::command]
pub async fn renamer_delete_preset(state: State<'_, AppState>, name: String) -> Result<(), String> {
    state.rename_presets.delete(&name).map_err(err_string)
}

#[tauri::command]
pub async fn renamer_auto_save_pipeline(
    state: State<'_, AppState>,
    pipeline: renamer::RulePipeline,
) -> Result<(), String> {
    state.rename_presets.auto_save(&pipeline).map_err(err_string)
}

#[tauri::command]
pub async fn renamer_auto_load_pipeline(
    state: State<'_, AppState>,
) -> Result<Option<renamer::RulePipeline>, String> {
    state.rename_presets.auto_load().map_err(err_string)
}

#[tauri::command]
pub async fn list_logs(state: State<'_, AppState>) -> Result<Vec<crate::log_store::LogEntry>, String> {
    Ok(state.logs.list())
}

#[tauri::command]
pub async fn clear_logs(state: State<'_, AppState>) -> Result<(), String> {
    state.logs.clear();
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryDto {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub file_size: Option<u64>,
    pub modified_at: Option<String>,
}

#[tauri::command]
pub async fn list_directory(path: String) -> Result<Vec<DirectoryEntryDto>, String> {
    let root = std::path::PathBuf::from(&path);
    if !root.is_dir() {
        return Err("path is not a directory".into());
    }
    let mut out = Vec::new();
    let rd = std::fs::read_dir(&root).map_err(err_string)?;
    for entry in rd {
        let entry = entry.map_err(err_string)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        let child = entry.path();
        let meta = entry.metadata().ok();
        let is_directory = meta.as_ref().map(|m| m.is_dir()).unwrap_or(false);
        let file_size = meta
            .as_ref()
            .filter(|m| m.is_file())
            .map(|m| m.len());
        let modified_at = meta
            .and_then(|m| m.modified().ok())
            .map(|t| {
                let dt: chrono::DateTime<chrono::Local> = t.into();
                dt.format("%Y-%m-%d").to_string()
            });
        out.push(DirectoryEntryDto {
            name,
            path: child.to_string_lossy().into_owned(),
            is_directory,
            file_size,
            modified_at,
        });
    }
    out.sort_by(|a, b| match (a.is_directory, b.is_directory) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

const MAX_RENAMER_FILES: usize = 5_000;

fn collect_paths_into(
    path: &std::path::Path,
    out: &mut Vec<renamer::FileEntry>,
) -> std::io::Result<()> {
    if path.is_file() {
        out.push(renamer::FileEntry::new(path));
        return Ok(());
    }
    if !path.is_dir() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with('.') {
            continue;
        }
        if child.is_dir() {
            collect_paths_into(&child, out)?;
        } else if child.is_file() {
            out.push(renamer::FileEntry::new(&child));
        }
        if out.len() > MAX_RENAMER_FILES {
            break;
        }
    }
    Ok(())
}

fn err_string(err: impl ToString) -> String {
    err.to_string()
}
