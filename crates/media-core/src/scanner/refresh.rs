use std::collections::HashSet;

use crate::models::{Library, MediaType};
use crate::nfo::import_nfo_for_item;
use crate::AppDatabase;
use crate::DatabaseError;

use super::movies::{scan_movies, ScanProgress};
use super::shows::scan_shows;

#[derive(Debug, Clone, Default)]
pub struct RefreshReport {
    pub new_item_count: usize,
    pub discovered_media_count: u32,
    pub imported_nfo_count: usize,
}

/// Full refresh for M1. Incremental mode lands in M4.
/// Flow mirrors Swift `LibraryRefreshService.scanLibrary`:
/// scan → persist new items → importNFOForItem(trulyNewItems only).
pub fn refresh_library(
    db: &AppDatabase,
    library: &Library,
    excluded_folders: &[String],
    mut on_progress: impl FnMut(ScanProgress),
) -> Result<RefreshReport, RefreshError> {
    let excluded: HashSet<String> = excluded_folders
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();

    let mut report = RefreshReport::default();

    match library.media_type {
        MediaType::Movie => {
            let existing: HashSet<String> = db
                .list_media_file_paths(&library.id)?
                .into_iter()
                .collect();
            let mut discovered = 0u32;
            let result = scan_movies(library, &existing, &excluded, |p| {
                discovered = p.discovered_count;
                on_progress(p);
            })?;
            db.insert_media_items(&result.new_items)?;
            report.new_item_count = result.new_items.len();
            report.discovered_media_count = discovered;

            for item in &result.new_items {
                if import_nfo_for_item(db, item)? {
                    report.imported_nfo_count += 1;
                }
            }
        }
        MediaType::TvShow | MediaType::Anime => {
            let existing: HashSet<String> = db
                .list_media_folder_paths(&library.id)?
                .into_iter()
                .collect();
            let mut discovered = 0u32;
            let result = scan_shows(library, &existing, &excluded, |p| {
                discovered = p.discovered_count;
                on_progress(p);
            })?;
            db.insert_media_items(&result.new_items)?;
            for item in &result.new_items {
                if let Some(episodes) = result.episodes.get(&item.id) {
                    db.insert_show_episodes(&item.id, episodes)?;
                }
            }
            report.new_item_count = result.new_items.len();
            report.discovered_media_count = discovered;

            for item in &result.new_items {
                if import_nfo_for_item(db, item)? {
                    report.imported_nfo_count += 1;
                }
            }
        }
    }

    Ok(report)
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
