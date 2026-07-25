use std::collections::HashSet;

use crate::models::{Library, MediaType};
use crate::nfo::import_nfo_for_item;
use crate::AppDatabase;
use crate::DatabaseError;

use super::incremental::{
    known_directories_unchanged, plan_directories, touch_scan_states,
};
use super::movies::{scan_movies_under, ScanProgress};
use super::shows::scan_shows_under;

#[derive(Debug, Clone, Default)]
pub struct RefreshReport {
    pub new_item_count: usize,
    pub discovered_media_count: u32,
    pub imported_nfo_count: usize,
    pub removed_item_count: usize,
    /// SCAN-14: no directory changes; skipped media file enumeration.
    pub early_exit: bool,
}

/// Incremental refresh (SCAN-11…14 / 13).
/// Flow: directory mtime plan → prune removed → scan changed roots → persist → upsert scan state.
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

    on_progress(ScanProgress {
        discovered_count: 0,
        current_path: library.root_path.clone(),
        current_name: "scan.checking".into(),
    });

    let previous = db.list_scan_states(&library.id)?;
    let mut report = RefreshReport::default();

    // Fast path: every known dir still present with unchanged mtime → skip WalkDir.
    if known_directories_unchanged(&previous)? {
        tracing::info!(
            library_id = %library.id,
            dirs = previous.len(),
            "refresh fast early-exit (known dirs unchanged)"
        );
        db.upsert_scan_states(&touch_scan_states(&previous))?;
        report.early_exit = true;
        on_progress(ScanProgress {
            discovered_count: 0,
            current_path: library.root_path.clone(),
            current_name: "scan.unchanged".into(),
        });
        return Ok(report);
    }

    let plan = plan_directories(library, &excluded, &previous)?;

    tracing::info!(
        library_id = %library.id,
        bootstrap = plan.bootstrap,
        live = plan.live.len(),
        added = plan.added.len(),
        changed = plan.changed.len(),
        removed = plan.removed.len(),
        "refresh directory plan"
    );

    // SCAN-13: directories gone from disk → remove rooted media + scan state.
    if plan.has_removals() {
        report.removed_item_count =
            db.delete_media_items_rooted_under(&library.id, &plan.removed)?;
        let _ = db.delete_scan_states(&library.id, &plan.removed)?;
    }

    // SCAN-14: nothing added/changed (and bootstrap false) → persist plan, no file walk.
    if !plan.needs_file_scan() {
        db.upsert_scan_states(&plan.to_scan_states(&library.id))?;
        report.early_exit = true;
        on_progress(ScanProgress {
            discovered_count: 0,
            current_path: library.root_path.clone(),
            current_name: "scan.unchanged".into(),
        });
        return Ok(report);
    }

    // SCAN-14: empty scan_roots after prune → persist without file walk.
    let scan_roots = plan.scan_roots();
    if scan_roots.is_empty() {
        db.upsert_scan_states(&plan.to_scan_states(&library.id))?;
        report.early_exit = true;
        on_progress(ScanProgress {
            discovered_count: 0,
            current_path: library.root_path.clone(),
            current_name: "scan.unchanged".into(),
        });
        return Ok(report);
    }

    match library.media_type {
        MediaType::Movie => {
            let existing: HashSet<String> = db
                .list_media_file_paths(&library.id)?
                .into_iter()
                .collect();
            let mut discovered = 0u32;
            let result =
                scan_movies_under(library, &scan_roots, &existing, &excluded, |p| {
                    discovered = p.discovered_count;
                    on_progress(p);
                })?;
            db.insert_media_items(&result.new_items)?;
            report.new_item_count = result.new_items.len();
            report.discovered_media_count = discovered;

            // Persist incremental state before NFO so a later NFO failure
            // does not force bootstrap full-scan on the next refresh.
            persist_fresh_scan_states(db, library, &excluded)?;

            for item in &result.new_items {
                if import_nfo_for_item(db, item)? {
                    report.imported_nfo_count += 1;
                }
            }
        }
        MediaType::TvShow | MediaType::Anime => {
            let existing_items = db.list_media_items(&library.id)?;
            let existing: HashSet<String> = existing_items
                .iter()
                .map(|i| i.folder_path.clone())
                .collect();
            let mut discovered = 0u32;
            let result =
                scan_shows_under(library, &scan_roots, &existing, &excluded, |p| {
                    discovered = p.discovered_count;
                    on_progress(p);
                })?;

            let (result, absorbed) =
                super::shows::absorb_flat_seasons_into_existing(result, &existing_items);

            db.insert_media_items(&result.new_items)?;
            for item in &result.new_items {
                if let Some(episodes) = result.episodes.get(&item.id) {
                    db.insert_show_episodes(&item.id, episodes)?;
                }
            }
            for (existing_id, episodes) in &absorbed {
                db.insert_show_episodes(existing_id, episodes)?;
            }

            // Nested `Show/Season XX`: files under an existing show root are skipped by
            // scan_shows_under — resync touched shows so new seasons land on the same item.
            for item in &existing_items {
                if super::shows::existing_show_touched_by_roots(&item.folder_path, &scan_roots) {
                    resync_show_episodes(db, item, &excluded)?;
                }
            }

            report.new_item_count = result.new_items.len();
            report.discovered_media_count = discovered;

            persist_fresh_scan_states(db, library, &excluded)?;

            for item in &result.new_items {
                if import_nfo_for_item(db, item)? {
                    report.imported_nfo_count += 1;
                }
            }
        }
    }

    Ok(report)
}

#[derive(Debug, Clone, Default)]
pub struct ItemRefreshReport {
    pub refreshed: usize,
    pub removed: usize,
    pub imported_nfo: usize,
}

/// SCAN-15: refresh selected items from disk.
/// Missing primary path → delete DB row (no synthetic "missing" status).
pub fn refresh_items(
    db: &AppDatabase,
    item_ids: &[String],
    excluded_folders: &[String],
) -> Result<ItemRefreshReport, RefreshError> {
    let excluded: HashSet<String> = excluded_folders
        .iter()
        .map(|s| s.to_ascii_lowercase())
        .collect();
    let mut report = ItemRefreshReport::default();

    for id in item_ids {
        let Some(item) = db.get_media_item(id)? else {
            continue;
        };
        let primary_ok = match item.media_type {
            MediaType::Movie => {
                !item.file_path.is_empty() && std::path::Path::new(&item.file_path).is_file()
            }
            MediaType::TvShow | MediaType::Anime => {
                !item.folder_path.is_empty() && std::path::Path::new(&item.folder_path).is_dir()
            }
        };
        if !primary_ok {
            db.delete_media_item(&item.id)?;
            report.removed += 1;
            continue;
        }

        if matches!(item.media_type, MediaType::TvShow | MediaType::Anime) {
            resync_show_episodes(db, &item, &excluded)?;
        }

        if import_nfo_for_item(db, &item)? {
            report.imported_nfo += 1;
        }
        report.refreshed += 1;
    }

    Ok(report)
}

fn resync_show_episodes(
    db: &AppDatabase,
    item: &crate::models::MediaItem,
    excluded: &HashSet<String>,
) -> Result<(), RefreshError> {
    use super::shows::discover_episodes_in_show;
    use std::path::Path;

    let folder = Path::new(&item.folder_path);
    let discovered = discover_episodes_in_show(folder, excluded)?;
    let discovered_keys: HashSet<(i32, i32)> = discovered
        .iter()
        .map(|e| (e.season, e.episode))
        .collect();
    let path_by_key: std::collections::HashMap<(i32, i32), &str> = discovered
        .iter()
        .map(|e| ((e.season, e.episode), e.file_path.as_str()))
        .collect();

    for season in db.fetch_seasons(&item.id)? {
        for ep in db.fetch_episodes(&season.id)? {
            let key = (season.season_number, ep.episode_number);
            if !discovered_keys.contains(&key) {
                // File gone → drop episode row (do not invent missing status).
                let _ = db.delete_episode(&ep.id)?;
                continue;
            }
            if let Some(path) = path_by_key.get(&key) {
                if ep.file_path != *path {
                    db.update_episode_file_path(&ep.id, path)?;
                }
            }
        }
    }

    // Insert newly discovered season/episode rows.
    let mut to_insert = Vec::new();
    let seasons = db.fetch_seasons(&item.id)?;
    for ep in &discovered {
        let season_id = format!("{}_S{}", item.id, ep.season);
        let has_season = seasons.iter().any(|s| s.season_number == ep.season);
        if !has_season {
            to_insert.push(ep.clone());
            continue;
        }
        let eps = db.fetch_episodes(&season_id)?;
        if !eps.iter().any(|e| e.episode_number == ep.episode) {
            to_insert.push(ep.clone());
        }
    }
    if !to_insert.is_empty() {
        db.insert_show_episodes(&item.id, &to_insert)?;
    }
    Ok(())
}

fn persist_fresh_scan_states(
    db: &AppDatabase,
    library: &Library,
    excluded: &HashSet<String>,
) -> Result<(), RefreshError> {
    let fresh = plan_directories(library, excluded, &[])?;
    let live_paths: HashSet<String> = fresh.live.keys().cloned().collect();
    let existing = db.list_scan_states(&library.id)?;
    let stale: Vec<String> = existing
        .into_iter()
        .map(|s| s.directory_path)
        .filter(|p| !live_paths.contains(p))
        .collect();
    if !stale.is_empty() {
        let _ = db.delete_scan_states(&library.id, &stale)?;
    }
    db.upsert_scan_states(&fresh.to_scan_states(&library.id))?;
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum RefreshError {
    #[error(transparent)]
    Database(#[from] DatabaseError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MediaType;
    use tempfile::tempdir;

    fn movie_library(root: &std::path::Path) -> Library {
        Library::new("Movies", root.display().to_string(), MediaType::Movie)
    }

    #[test]
    fn bootstrap_then_early_exit_on_unchanged() {
        let dir = tempdir().unwrap();
        let movie = dir.path().join("Dune (2021)");
        std::fs::create_dir_all(&movie).unwrap();
        std::fs::write(movie.join("Dune.2021.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();

        let first = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert!(!first.early_exit);
        assert_eq!(first.new_item_count, 1);
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 1);

        let second = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert!(second.early_exit);
        assert_eq!(second.new_item_count, 0);
        assert_eq!(second.discovered_media_count, 0);
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 1);
    }

    #[test]
    fn detects_new_movie_folder() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("A (2020)");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("A.2020.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();
        refresh_library(&db, &library, &[], |_| {}).unwrap();

        // Ensure mtime can advance on some filesystems.
        std::thread::sleep(std::time::Duration::from_millis(20));
        let b = dir.path().join("B (2021)");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::write(b.join("B.2021.mkv"), b"x").unwrap();

        let report = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert!(!report.early_exit);
        assert_eq!(report.new_item_count, 1);
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 2);
    }

    #[test]
    fn second_refresh_immediate_should_early_exit() {
        let dir = tempdir().unwrap();
        let movie = dir.path().join("Blade (1998)");
        std::fs::create_dir_all(&movie).unwrap();
        std::fs::write(movie.join("Blade.1998.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();

        let first = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert_eq!(first.new_item_count, 1);
        let states = db.list_scan_states(&library.id).unwrap();
        assert!(
            !states.is_empty(),
            "scan state should persist after first refresh"
        );

        let second = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert!(
            second.early_exit,
            "second refresh must early-exit; states={}, discovered={}, new={}, removed={}",
            states.len(),
            second.discovered_media_count,
            second.new_item_count,
            second.removed_item_count
        );
        assert_eq!(second.discovered_media_count, 0);
    }

    #[test]
    fn empty_refresh_many_dirs() {
        // ~300 movie folders: second refresh must early-exit without rediscovering media.
        let dir = tempdir().unwrap();
        for i in 0..300 {
            let movie = dir.path().join(format!("Title{i:03} ({})", 2000 + (i % 20)));
            std::fs::create_dir_all(&movie).unwrap();
            std::fs::write(movie.join(format!("Title{i:03}.mkv")), b"x").unwrap();
        }

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();

        let first = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert!(!first.early_exit);
        assert_eq!(first.new_item_count, 300);
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 300);

        let started = std::time::Instant::now();
        let second = refresh_library(&db, &library, &[], |_| {}).unwrap();
        let elapsed = started.elapsed();
        assert!(
            second.early_exit,
            "unchanged library must early-exit; discovered={}",
            second.discovered_media_count
        );
        assert_eq!(second.discovered_media_count, 0);
        assert_eq!(second.new_item_count, 0);
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 300);
        // Soft budget: empty refresh of 300 dirs should stay well under a few seconds locally.
        assert!(
            elapsed.as_secs() < 15,
            "empty refresh too slow: {elapsed:?}"
        );
    }

    #[test]
    fn scan_state_persists_even_if_nfo_step_would_run_after() {
        // Guarantees state exists before NFO loop so next refresh can early-exit.
        let dir = tempdir().unwrap();
        let movie = dir.path().join("X (2000)");
        std::fs::create_dir_all(&movie).unwrap();
        std::fs::write(movie.join("X.2000.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();
        refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert!(!db.list_scan_states(&library.id).unwrap().is_empty());
    }

    #[test]
    fn removes_media_when_folder_deleted() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("Gone (2019)");
        std::fs::create_dir_all(&a).unwrap();
        std::fs::write(a.join("Gone.2019.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();
        refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 1);

        std::fs::remove_dir_all(&a).unwrap();
        // Touch library root so parent mtime may change; removal is detected via missing state paths.
        let _ = std::fs::metadata(dir.path());

        let report = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert_eq!(report.removed_item_count, 1);
        assert!(db.list_media_items(&library.id).unwrap().is_empty());
    }

    #[test]
    fn refresh_items_deletes_when_primary_missing() {
        let dir = tempdir().unwrap();
        let movie = dir.path().join("GoneItem (2019)");
        std::fs::create_dir_all(&movie).unwrap();
        let file = movie.join("GoneItem.2019.mkv");
        std::fs::write(&file, b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = movie_library(dir.path());
        db.insert_library(&library).unwrap();
        refresh_library(&db, &library, &[], |_| {}).unwrap();
        let items = db.list_media_items(&library.id).unwrap();
        assert_eq!(items.len(), 1);
        let id = items[0].id.clone();

        std::fs::remove_file(&file).unwrap();
        std::fs::remove_dir_all(&movie).unwrap();

        let report = refresh_items(&db, &[id], &[]).unwrap();
        assert_eq!(report.removed, 1);
        assert_eq!(report.refreshed, 0);
        assert!(db.list_media_items(&library.id).unwrap().is_empty());
    }

    #[test]
    fn flat_season_merges_into_existing_on_second_refresh() {
        let dir = tempdir().unwrap();
        let s1 = dir.path().join("后室 第 1 季");
        std::fs::create_dir_all(&s1).unwrap();
        std::fs::write(s1.join("S01E01.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();

        let first = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert_eq!(first.new_item_count, 1);
        assert_eq!(db.list_media_items(&library.id).unwrap().len(), 1);

        std::thread::sleep(std::time::Duration::from_millis(20));
        let s2 = dir.path().join("后室 第 2 季");
        std::fs::create_dir_all(&s2).unwrap();
        std::fs::write(s2.join("S02E01.mkv"), b"x").unwrap();

        let second = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert_eq!(second.new_item_count, 0, "season 2 must merge, not create a new show");
        let items = db.list_media_items(&library.id).unwrap();
        assert_eq!(items.len(), 1);
        let seasons = db.fetch_seasons(&items[0].id).unwrap();
        let nums: std::collections::HashSet<_> =
            seasons.iter().map(|s| s.season_number).collect();
        assert!(nums.contains(&1));
        assert!(nums.contains(&2));
    }

    #[test]
    fn nested_season_resyncs_into_existing_show() {
        let dir = tempdir().unwrap();
        let show = dir.path().join("Andor");
        let s1 = show.join("Season 01");
        std::fs::create_dir_all(&s1).unwrap();
        std::fs::write(s1.join("Andor.S01E01.mkv"), b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();
        assert_eq!(
            refresh_library(&db, &library, &[], |_| {})
                .unwrap()
                .new_item_count,
            1
        );

        std::thread::sleep(std::time::Duration::from_millis(20));
        let s2 = show.join("Season 02");
        std::fs::create_dir_all(&s2).unwrap();
        std::fs::write(s2.join("Andor.S02E01.mkv"), b"x").unwrap();

        let second = refresh_library(&db, &library, &[], |_| {}).unwrap();
        assert_eq!(second.new_item_count, 0);
        let items = db.list_media_items(&library.id).unwrap();
        assert_eq!(items.len(), 1);
        let seasons = db.fetch_seasons(&items[0].id).unwrap();
        let nums: std::collections::HashSet<_> =
            seasons.iter().map(|s| s.season_number).collect();
        assert!(nums.contains(&1) && nums.contains(&2));
    }
}
