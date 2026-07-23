use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use crate::config::AppConfig;
use crate::state::{AppState, AppStatusDto, CratesDto};
use crate::task_queue::{TaskSnapshot, TaskStatus};

#[tauri::command]
pub async fn app_status(state: State<'_, AppState>) -> Result<AppStatusDto, String> {
    let library_count = state.db.library_count().map_err(err_string)?;
    let config = state.config.lock().await.config.clone();
    Ok(AppStatusDto {
        app_name: "ScrapeX".into(),
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
    let _ = app.emit("task-updated", &snapshot);

    let tasks = Arc::clone(&state.tasks);
    let app2 = app.clone();
    let id = snapshot.id.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            let list = tasks.list().await;
            if let Some(current) = list.into_iter().find(|t| t.id == id) {
                let _ = app2.emit("task-updated", &current);
                if matches!(
                    current.status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    break;
                }
            } else {
                break;
            }
        }
    });
    Ok(snapshot)
}

#[tauri::command]
pub async fn cancel_task(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    Ok(state.tasks.cancel(&id).await)
}

fn err_string(err: impl ToString) -> String {
    err.to_string()
}
