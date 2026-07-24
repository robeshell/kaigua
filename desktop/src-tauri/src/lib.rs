mod commands;
mod config;
mod state;
mod task_queue;

use state::AppState;
use tracing_subscriber::EnvFilter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::bootstrap().expect("failed to bootstrap kaigua app state"))
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::get_config,
            commands::save_config,
            commands::list_libraries,
            commands::add_library,
            commands::rename_library,
            commands::delete_library,
            commands::list_media_items,
            commands::list_media_page,
            commands::get_media_detail,
            commands::resolve_poster_thumbnail,
            commands::refresh_library,
            commands::scrape_library,
            commands::scrape_items,
            commands::search_match_candidates,
            commands::apply_manual_match,
            commands::list_tasks,
            commands::enqueue_smoke_task,
            commands::cancel_task,
        ])
        .run(tauri::generate_context!())
        .expect("error while running kaigua");
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
    tracing::info!("kaigua tracing initialized");
}
