//! Apply media rename templates to scraped items (M3).
//! Movie + TV show folder/file rename after scrape.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use media_core::{
    companion_suffix, AppDatabase, CollisionPolicy, FileNameParser, FilesystemService, MediaItem,
    MediaType, ScannedEpisode, ScrapedStatus, TvEpisode, TvSeason, COMPANION_EXTENSIONS,
};
use thiserror::Error;

use crate::template::TemplateEngine;

#[derive(Debug, Clone)]
pub struct RenameTemplates {
    pub movie_folder: String,
    pub movie_file: String,
    pub tv_show_folder: String,
    pub season_folder: String,
    pub episode_file: String,
}

impl Default for RenameTemplates {
    fn default() -> Self {
        Self {
            movie_folder: TemplateEngine::MOVIE_FOLDER.into(),
            movie_file: TemplateEngine::MOVIE_FILE.into(),
            tv_show_folder: TemplateEngine::TV_SHOW_FOLDER.into(),
            season_folder: TemplateEngine::SEASON_FOLDER.into(),
            episode_file: TemplateEngine::EPISODE_FILE.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum RenameError {
    #[error("item is not scraped")]
    NotScraped,
    #[error("organize season folders is only for tv/anime")]
    NotTvShow,
    #[error("empty rename target")]
    EmptyTarget,
    #[error("destination already exists: {0}")]
    DestinationExists(PathBuf),
    #[error("path not found: {0}")]
    NotFound(PathBuf),
    #[error("database: {0}")]
    Database(String),
    #[error("filesystem: {0}")]
    Filesystem(String),
}

pub fn rename_after_scrape(
    db: &AppDatabase,
    item: &MediaItem,
    templates: &RenameTemplates,
) -> Result<(), RenameError> {
    rename_after_scrape_with_options(db, item, templates, false)
}

/// Apply templates; when `create_season_folders` is true, ensure TV episodes live under Season XX.
pub fn rename_after_scrape_with_options(
    db: &AppDatabase,
    item: &MediaItem,
    templates: &RenameTemplates,
    create_season_folders: bool,
) -> Result<(), RenameError> {
    if item.status != ScrapedStatus::Scraped {
        return Err(RenameError::NotScraped);
    }
    match item.media_type {
        MediaType::Movie => rename_movie(db, item, templates),
        MediaType::TvShow | MediaType::Anime => {
            rename_tv_show(db, item, templates, create_season_folders)
        }
    }
}

/// Move episode files into `Season XX` folders under the show root (RENAME-T-06).
pub fn organize_season_folders(
    db: &AppDatabase,
    item: &MediaItem,
    templates: &RenameTemplates,
) -> Result<(), RenameError> {
    if item.status != ScrapedStatus::Scraped {
        return Err(RenameError::NotScraped);
    }
    if !matches!(item.media_type, MediaType::TvShow | MediaType::Anime) {
        return Err(RenameError::NotTvShow);
    }
    rename_tv_show(db, item, templates, true)
}

/// Merge duplicate TV/anime items that share TMDB/TVDB/Bangumi (or title+year).
/// Returns how many source items were absorbed into a canonical show.
pub fn consolidate_library_duplicate_shows(
    db: &AppDatabase,
    library_id: &str,
    templates: &RenameTemplates,
) -> Result<usize, RenameError> {
    let items = db
        .list_media_items(library_id)
        .map_err(|e| RenameError::Database(e.to_string()))?;
    let mut ranked: Vec<MediaItem> = items
        .into_iter()
        .filter(|i| matches!(i.media_type, MediaType::TvShow | MediaType::Anime))
        .collect();
    // Merge low-score release dumps into high-score canonical roots first.
    ranked.sort_by_key(|a| show_root_score(a));
    let mut merged = 0usize;
    for item in ranked {
        let Some(fresh) = db
            .get_media_item(&item.id)
            .map_err(|e| RenameError::Database(e.to_string()))?
        else {
            continue;
        };
        if consolidate_show_item(db, &fresh, templates)? {
            merged += 1;
        }
    }
    Ok(merged)
}

/// If `item` is a duplicate of a better canonical show, merge it in and delete `item`.
pub fn consolidate_show_item(
    db: &AppDatabase,
    item: &MediaItem,
    templates: &RenameTemplates,
) -> Result<bool, RenameError> {
    let Some(target) = find_canonical_show_duplicate(db, item)? else {
        return Ok(false);
    };
    merge_show_into(db, item, &target, templates)?;
    Ok(true)
}

fn find_canonical_show_duplicate(
    db: &AppDatabase,
    item: &MediaItem,
) -> Result<Option<MediaItem>, RenameError> {
    if !matches!(item.media_type, MediaType::TvShow | MediaType::Anime) {
        return Ok(None);
    }
    let meta = db
        .fetch_metadata(&item.id)
        .map_err(|e| RenameError::Database(e.to_string()))?;
    let tmdb = meta
        .as_ref()
        .and_then(|m| m.tmdb_id.clone())
        .filter(|s| !s.is_empty());
    let bangumi = meta
        .as_ref()
        .and_then(|m| m.bangumi_id.clone())
        .filter(|s| !s.is_empty());
    let tvdb = meta
        .as_ref()
        .and_then(|m| m.tvdb_id.clone())
        .filter(|s| !s.is_empty());
    let title_key = normalize_show_title(&item.title);
    let year = item.year;

    let others = db
        .list_media_items(&item.library_id)
        .map_err(|e| RenameError::Database(e.to_string()))?;
    let mut matches = Vec::new();
    for other in others {
        if other.id == item.id {
            continue;
        }
        if !matches!(other.media_type, MediaType::TvShow | MediaType::Anime) {
            continue;
        }
        // Never merge a show into one of its own nested subfolders (or vice versa)
        // unless identity matches — nested junk under the show root is a separate issue.
        let om = db
            .fetch_metadata(&other.id)
            .map_err(|e| RenameError::Database(e.to_string()))?;
        let same_provider = tmdb
            .as_ref()
            .zip(om.as_ref().and_then(|m| m.tmdb_id.as_ref()))
            .is_some_and(|(a, b)| a == b)
            || bangumi
                .as_ref()
                .zip(om.as_ref().and_then(|m| m.bangumi_id.as_ref()))
                .is_some_and(|(a, b)| a == b)
            || tvdb
                .as_ref()
                .zip(om.as_ref().and_then(|m| m.tvdb_id.as_ref()))
                .is_some_and(|(a, b)| a == b);
        let same_title_year = !title_key.is_empty()
            && title_key == normalize_show_title(&other.title)
            && year.is_some()
            && year == other.year
            && item.status == ScrapedStatus::Scraped
            && other.status == ScrapedStatus::Scraped;
        if same_provider || same_title_year {
            matches.push(other);
        }
    }
    if matches.is_empty() {
        return Ok(None);
    }

    let mut all = matches;
    all.push(item.clone());
    all.sort_by(|a, b| {
        show_root_score(b)
            .cmp(&show_root_score(a))
            .then_with(|| a.added_at.cmp(&b.added_at))
    });
    let best = all.remove(0);
    if best.id == item.id {
        Ok(None)
    } else {
        Ok(Some(best))
    }
}

fn merge_show_into(
    db: &AppDatabase,
    source: &MediaItem,
    target: &MediaItem,
    templates: &RenameTemplates,
) -> Result<(), RenameError> {
    if source.id == target.id {
        return Ok(());
    }
    let target_root = PathBuf::from(&target.folder_path);
    if !target_root.is_dir() {
        return Err(RenameError::NotFound(target_root));
    }
    let fs = FilesystemService::new();
    let mut to_insert: Vec<ScannedEpisode> = Vec::new();

    let seasons = db
        .fetch_seasons(&source.id)
        .map_err(|e| RenameError::Database(e.to_string()))?;
    for season in seasons {
        let episodes = db
            .fetch_episodes(&season.id)
            .map_err(|e| RenameError::Database(e.to_string()))?;
        let mut season_values = HashMap::new();
        season_values.insert("season".into(), season.season_number.to_string());
        let season_folder_name = TemplateEngine::sanitize_filename(&TemplateEngine::render(
            &templates.season_folder,
            &season_values,
        ));
        let dest_season = if season_folder_name.is_empty() {
            target_root.clone()
        } else {
            let dir = target_root.join(&season_folder_name);
            if !dir.exists() {
                fs.create_directory(&dir)
                    .map_err(|e| RenameError::Filesystem(e.to_string()))?;
            }
            dir
        };

        for ep in episodes {
            if ep.file_path.is_empty() {
                continue;
            }
            let src = PathBuf::from(&ep.file_path);
            if !src.is_file() {
                continue;
            }
            let file_name = src
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            if file_name.is_empty() {
                continue;
            }
            let stem = src
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let from_dir = src
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| dest_season.clone());
            let dest = dest_season.join(&file_name);
            if src != dest {
                if dest.exists() {
                    continue;
                }
                fs.move_item(&src, &dest, CollisionPolicy::Fail)
                    .map_err(|e| RenameError::Filesystem(e.to_string()))?;
                move_companions(&fs, &from_dir, &dest_season, &stem);
            }
            to_insert.push(ScannedEpisode {
                season: season.season_number,
                episode: ep.episode_number,
                file_path: dest.to_string_lossy().into_owned(),
                title: ep.title.clone().unwrap_or_default(),
            });
        }
    }

    if !to_insert.is_empty() {
        db.insert_show_episodes(&target.id, &to_insert)
            .map_err(|e| RenameError::Database(e.to_string()))?;
    }
    db.delete_media_item(&source.id)
        .map_err(|e| RenameError::Database(e.to_string()))?;
    remove_merged_source_folder(&fs, source, target);
    Ok(())
}

/// Trash (fallback hard-delete) the absorbed show folder after episodes were moved.
fn remove_merged_source_folder(fs: &FilesystemService, source: &MediaItem, target: &MediaItem) {
    let source_root = PathBuf::from(&source.folder_path);
    let target_root = PathBuf::from(&target.folder_path);
    if !source_root.is_dir() {
        return;
    }
    // Never touch the canonical root or nested parent/child paths.
    if source_root == target_root
        || source_root.starts_with(&target_root)
        || target_root.starts_with(&source_root)
    {
        return;
    }
    // Prefer trash; always fall back to hard delete so leftovers don't linger.
    if fs.trash_item(&source_root).is_err() {
        let _ = fs.remove_item(&source_root);
    }
}

fn normalize_show_title(title: &str) -> String {
    title
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn show_root_score(item: &MediaItem) -> i32 {
    let name = Path::new(&item.folder_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    let lower = name.to_ascii_lowercase();
    let mut score = 0i32;
    if lower.contains("web-dl")
        || lower.contains("webrip")
        || lower.contains("1080p")
        || lower.contains("720p")
        || lower.contains("2160p")
        || lower.contains("hdtv")
        || lower.contains("www.")
        || name.contains('【')
        || (name.contains('[') && name.contains(']'))
    {
        score -= 100;
    }
    if FileNameParser::extract_season_suffix(name).is_some() {
        score -= 20;
    }
    // Prefer classic `Title (Year)` show roots.
    if name.contains('(') && name.ends_with(')') {
        score += 40;
    }
    score
}

fn rename_movie(
    db: &AppDatabase,
    item: &MediaItem,
    templates: &RenameTemplates,
) -> Result<(), RenameError> {
    let values = build_values(db, item);
    let new_folder_name = TemplateEngine::sanitize_filename(&TemplateEngine::render(
        &templates.movie_folder,
        &values,
    ));
    let new_file_stem = TemplateEngine::sanitize_filename(&TemplateEngine::render(
        &templates.movie_file,
        &values,
    ));
    if new_folder_name.is_empty() || new_file_stem.is_empty() {
        return Err(RenameError::EmptyTarget);
    }

    let fs = FilesystemService::new();
    let folder = PathBuf::from(&item.folder_path);
    if !folder.exists() {
        return Err(RenameError::NotFound(folder));
    }

    let mut current_folder = folder.clone();
    let parent = folder
        .parent()
        .ok_or_else(|| RenameError::NotFound(folder.clone()))?;

    if folder.file_name().and_then(|n| n.to_str()).unwrap_or("") != new_folder_name {
        let dest = parent.join(&new_folder_name);
        if dest.exists() {
            return Err(RenameError::DestinationExists(dest));
        }
        fs.move_item(&folder, &dest, CollisionPolicy::Fail)
            .map_err(|e| RenameError::Filesystem(e.to_string()))?;
        current_folder = dest;
    }

    let mut new_file_path = item.file_path.clone();
    if !item.file_path.is_empty() {
        let old_video = PathBuf::from(&item.file_path);
        let ext = old_video
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("mkv");
        let old_stem = old_video
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let old_name = old_video
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let current_video = current_folder.join(&old_name);
        let new_video_name = format!("{new_file_stem}.{ext}");
        let new_video = current_folder.join(&new_video_name);

        if current_video.file_name() != new_video.file_name() {
            if !new_video.exists() && current_video.exists() {
                fs.move_item(&current_video, &new_video, CollisionPolicy::Fail)
                    .map_err(|e| RenameError::Filesystem(e.to_string()))?;
            }
            rename_companions(&fs, &current_folder, &old_stem, &new_file_stem);
        }
        new_file_path = new_video.to_string_lossy().into_owned();
    }

    db.update_media_paths(
        &item.id,
        &current_folder.to_string_lossy(),
        &new_file_path,
    )
    .map_err(|e| RenameError::Database(e.to_string()))?;

    Ok(())
}

fn rename_tv_show(
    db: &AppDatabase,
    item: &MediaItem,
    templates: &RenameTemplates,
    create_season_folders: bool,
) -> Result<(), RenameError> {
    let show_values = build_values(db, item);
    let seasons = db
        .fetch_seasons(&item.id)
        .map_err(|e| RenameError::Database(e.to_string()))?;

    let new_show_folder_name = TemplateEngine::sanitize_filename(&TemplateEngine::render(
        &templates.tv_show_folder,
        &show_values,
    ));
    if new_show_folder_name.is_empty() {
        return Err(RenameError::EmptyTarget);
    }

    let fs = FilesystemService::new();
    let show_folder = PathBuf::from(&item.folder_path);
    if !show_folder.exists() {
        return Err(RenameError::NotFound(show_folder));
    }

    let mut current_show = show_folder.clone();
    if show_folder
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        != new_show_folder_name
    {
        let parent = show_folder
            .parent()
            .ok_or_else(|| RenameError::NotFound(show_folder.clone()))?;
        let dest = parent.join(&new_show_folder_name);
        if dest.exists() {
            // Do not abort: still rename episodes / create Season XX under the
            // current folder. Aborting here skipped organize entirely (silent Ok).
        } else {
            fs.move_item(&show_folder, &dest, CollisionPolicy::Fail)
                .map_err(|e| RenameError::Filesystem(e.to_string()))?;
            current_show = dest;
        }
    }

    for season in &seasons {
        let episodes = db
            .fetch_episodes(&season.id)
            .map_err(|e| RenameError::Database(e.to_string()))?;
        let Some(first) = episodes.iter().find(|e| !e.file_path.is_empty()) else {
            continue;
        };

        let old_ep_path = PathBuf::from(&first.file_path);
        let old_season_dir = old_ep_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| show_folder.clone());

        let relative_season = old_season_dir
            .strip_prefix(&show_folder)
            .ok()
            .map(|p| p.to_path_buf());

        let mut current_season = if let Some(rel) = &relative_season {
            current_show.join(rel)
        } else {
            current_show.clone()
        };

        let mut season_values = show_values.clone();
        season_values.insert("season".into(), season.season_number.to_string());
        season_values.insert(
            "seasonTitle".into(),
            season.title.clone().unwrap_or_default(),
        );

        let new_season_folder_name = TemplateEngine::sanitize_filename(&TemplateEngine::render(
            &templates.season_folder,
            &season_values,
        ));

        // Create / rename into Season XX when requested (RENAME-T-06).
        if create_season_folders && !new_season_folder_name.is_empty() {
            let target_season = current_show.join(&new_season_folder_name);
            if current_season != target_season {
                if !target_season.exists() {
                    fs.create_directory(&target_season)
                        .map_err(|e| RenameError::Filesystem(e.to_string()))?;
                }
                // Move episode files (and companions) from current_season into target.
                for ep in &episodes {
                    if ep.file_path.is_empty() {
                        continue;
                    }
                    let old_ep = PathBuf::from(&ep.file_path);
                    let old_name = old_ep
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("")
                        .to_string();
                    if old_name.is_empty() {
                        continue;
                    }
                    let old_stem = old_ep
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    let src = if old_ep.starts_with(&show_folder) {
                        let rel = old_ep.strip_prefix(&show_folder).unwrap_or(&old_ep);
                        current_show.join(rel)
                    } else if old_ep.exists() {
                        old_ep.clone()
                    } else {
                        current_season.join(&old_name)
                    };
                    let from_dir = src
                        .parent()
                        .map(Path::to_path_buf)
                        .unwrap_or_else(|| current_season.clone());
                    let dest = target_season.join(&old_name);
                    if src != dest && src.exists() && !dest.exists() {
                        fs.move_item(&src, &dest, CollisionPolicy::Fail)
                            .map_err(|e| RenameError::Filesystem(e.to_string()))?;
                    }
                    // Bring sidecars even if a previous partial run already moved the video.
                    move_companions(&fs, &from_dir, &target_season, &old_stem);
                    if from_dir != current_season {
                        move_companions(&fs, &current_season, &target_season, &old_stem);
                    }
                    let new_ep_path = dest.to_string_lossy().into_owned();
                    if new_ep_path != ep.file_path {
                        let _ = db.update_episode_file_path(&ep.id, &new_ep_path);
                    }
                    sync_episode_still_path(
                        db,
                        ep,
                        &current_show,
                        &target_season,
                        &old_stem,
                        &old_stem,
                    );
                }
                current_season = target_season;
            }
        } else if !new_season_folder_name.is_empty()
            && current_season
                .parent()
                .map(|p| p == current_show.as_path())
                .unwrap_or(false)
            && current_season
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                != new_season_folder_name
        {
            // Rename season folder only when it is a direct child of the show folder.
            let new_season = current_show.join(&new_season_folder_name);
            if !new_season.exists() && current_season.exists() {
                fs.move_item(&current_season, &new_season, CollisionPolicy::Fail)
                    .map_err(|e| RenameError::Filesystem(e.to_string()))?;
                current_season = new_season;
            }
        }

        // Re-fetch episode paths after possible organize moves.
        let episodes = db
            .fetch_episodes(&season.id)
            .map_err(|e| RenameError::Database(e.to_string()))?;

        for ep in &episodes {
            if ep.file_path.is_empty() {
                continue;
            }
            let ep_values = build_episode_values(&show_values, season, ep);
            let new_ep_stem = TemplateEngine::sanitize_filename(&TemplateEngine::render(
                &templates.episode_file,
                &ep_values,
            ));
            if new_ep_stem.is_empty() {
                continue;
            }

            let old_ep = PathBuf::from(&ep.file_path);
            let ext = old_ep
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("mkv");
            let old_stem = old_ep
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string();
            let old_name = old_ep
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let current_ep = if old_ep.exists() {
                old_ep.clone()
            } else {
                current_season.join(&old_name)
            };
            let dest_dir = current_ep
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| current_season.clone());
            let new_ep_name = format!("{new_ep_stem}.{ext}");
            let new_ep = dest_dir.join(&new_ep_name);

            if current_ep.file_name() != new_ep.file_name() {
                if !new_ep.exists() && current_ep.exists() {
                    fs.move_item(&current_ep, &new_ep, CollisionPolicy::Fail)
                        .map_err(|e| RenameError::Filesystem(e.to_string()))?;
                }
                rename_companions(&fs, &dest_dir, &old_stem, &new_ep_stem);
            }

            let new_ep_path = new_ep.to_string_lossy().into_owned();
            if new_ep_path != ep.file_path {
                let _ = db.update_episode_file_path(&ep.id, &new_ep_path);
            }
            sync_episode_still_path(db, ep, &current_show, &dest_dir, &old_stem, &new_ep_stem);
        }
    }

    let show_path = current_show.to_string_lossy().into_owned();
    db.update_media_paths(&item.id, &show_path, &show_path)
        .map_err(|e| RenameError::Database(e.to_string()))?;

    Ok(())
}

fn build_values(db: &AppDatabase, item: &MediaItem) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let meta = db.fetch_metadata(&item.id).ok().flatten();
    values.insert("title".into(), item.title.clone());
    if let Some(ot) = &item.original_title {
        values.insert("originalTitle".into(), ot.clone());
    }
    if let Some(year) = item.year {
        values.insert("year".into(), year.to_string());
    }
    if let Some(meta) = meta {
        if let Some(g) = meta.genres.first() {
            values.insert("genre".into(), g.clone());
        }
        if let Some(r) = meta.rating {
            values.insert("rating".into(), format!("{r:.1}"));
        }
        if let Some(c) = meta.content_rating {
            values.insert("contentRating".into(), c);
        }
        if let Some(d) = meta.director {
            values.insert("director".into(), d);
        }
        if let Some(s) = meta.studio {
            values.insert("studio".into(), s);
        }
        if let Some(c) = meta.country {
            values.insert("country".into(), c);
        }
        if let Some(cn) = meta.collection_name {
            values.insert("collection".into(), cn);
        }
        if let Some(vc) = meta.video_codec {
            values.insert("videoCodec".into(), vc);
        }
        if let Some(vr) = meta.video_resolution {
            values.insert("videoResolution".into(), vr);
        }
        if let Some(ac) = meta.audio_codec {
            values.insert("audioCodec".into(), ac);
        }
        if let Some(ach) = meta.audio_channels {
            values.insert("audioChannels".into(), ach);
        }
        if let Some(i) = meta.imdb_id {
            values.insert("imdbId".into(), i);
        }
        if let Some(t) = meta.tmdb_id {
            values.insert("tmdbId".into(), t);
        }
    }
    values
}

fn build_episode_values(
    show_values: &HashMap<String, String>,
    season: &TvSeason,
    ep: &TvEpisode,
) -> HashMap<String, String> {
    let mut v = show_values.clone();
    v.insert("season".into(), season.season_number.to_string());
    v.insert(
        "seasonTitle".into(),
        season.title.clone().unwrap_or_default(),
    );
    v.insert("episode".into(), ep.episode_number.to_string());
    v.insert(
        "episodeTitle".into(),
        ep.title.clone().unwrap_or_default(),
    );
    if let Some(an) = ep.absolute_number {
        v.insert("absoluteNumber".into(), an.to_string());
    }
    if let Some(ad) = &ep.air_date {
        v.insert("airDate".into(), ad.clone());
    }
    v
}

fn rename_companions(fs: &FilesystemService, dir: &Path, old_stem: &str, new_stem: &str) {
    if old_stem == new_stem || old_stem.is_empty() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !COMPANION_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let Some(suffix) = companion_suffix(&stem, old_stem) else {
            continue;
        };
        let new_name = format!("{new_stem}{suffix}.{ext}");
        let dest = dir.join(new_name);
        if !dest.exists() {
            let _ = fs.move_item(&path, &dest, CollisionPolicy::Fail);
        }
    }
}

/// Move companion files that share `stem` from `from_dir` into `to_dir` (keep names).
fn move_companions(fs: &FilesystemService, from_dir: &Path, to_dir: &Path, stem: &str) {
    if from_dir == to_dir || stem.is_empty() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(from_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !COMPANION_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }
        let file_stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if companion_suffix(&file_stem, stem).is_none() {
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_owned())
            .unwrap_or_default();
        let dest = to_dir.join(name);
        if !dest.exists() {
            let _ = fs.move_item(&path, &dest, CollisionPolicy::Fail);
        }
    }
}

fn relative_to_show(path: &Path, show_root: &Path) -> String {
    path.strip_prefix(show_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

fn companion_dest_name(file_name: &str, old_stem: &str, new_stem: &str) -> Option<String> {
    let path = Path::new(file_name);
    let stem = path.file_stem()?.to_str()?;
    let ext = path.extension()?.to_str()?;
    let suffix = companion_suffix(stem, old_stem)?;
    Some(format!("{new_stem}{suffix}.{ext}"))
}

fn sync_episode_still_path(
    db: &AppDatabase,
    ep: &TvEpisode,
    show_root: &Path,
    dest_dir: &Path,
    old_stem: &str,
    new_stem: &str,
) {
    let Some(still) = ep.still_path.as_deref() else {
        // Discover scraped still left next to the episode after move/rename.
        for candidate in [
            format!("{new_stem}-thumb.jpg"),
            format!("{new_stem}_thumb.jpg"),
            format!("{old_stem}-thumb.jpg"),
            format!("{old_stem}_thumb.jpg"),
        ] {
            let abs = dest_dir.join(&candidate);
            if abs.is_file() {
                let rel = relative_to_show(&abs, show_root);
                let _ = db.update_episode_still_path(&ep.id, &rel);
                return;
            }
        }
        return;
    };

    let old_name = Path::new(still)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if old_name.is_empty() {
        return;
    }
    let new_name =
        companion_dest_name(old_name, old_stem, new_stem).unwrap_or_else(|| old_name.to_string());
    let new_abs = dest_dir.join(&new_name);
    if new_abs.is_file() {
        let rel = relative_to_show(&new_abs, show_root);
        if rel != still {
            let _ = db.update_episode_still_path(&ep.id, &rel);
        }
        return;
    }
    let alt = dest_dir.join(old_name);
    if alt.is_file() {
        let rel = relative_to_show(&alt, show_root);
        if rel != still {
            let _ = db.update_episode_still_path(&ep.id, &rel);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use media_core::scanner::ScannedEpisode;
    use media_core::{Library, MediaType, ScrapedStatus};
    use tempfile::tempdir;

    #[test]
    fn organizes_flat_episodes_into_season_xx() {
        let dir = tempdir().unwrap();
        let show = dir.path().join("Andor (2022)");
        std::fs::create_dir_all(&show).unwrap();
        let ep1 = show.join("Andor.S01E01.mkv");
        let ep2 = show.join("Andor.S01E02.mkv");
        std::fs::write(&ep1, b"x").unwrap();
        std::fs::write(&ep2, b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();
        let item = MediaItem::new_show(
            MediaType::TvShow,
            "Andor",
            Some(2022),
            show.display().to_string(),
            library.id.clone(),
            ScrapedStatus::Scraped,
        );
        db.insert_media_items(&[item.clone()]).unwrap();
        db.insert_show_episodes(
            &item.id,
            &[
                ScannedEpisode {
                    season: 1,
                    episode: 1,
                    file_path: ep1.display().to_string(),
                    title: "Ep1".into(),
                },
                ScannedEpisode {
                    season: 1,
                    episode: 2,
                    file_path: ep2.display().to_string(),
                    title: "Ep2".into(),
                },
            ],
        )
        .unwrap();

        organize_season_folders(&db, &item, &RenameTemplates::default()).unwrap();

        let season_dir = show.join("Season 01");
        assert!(season_dir.is_dir());
        let eps = db.fetch_episodes(&format!("{}_S1", item.id)).unwrap();
        assert_eq!(eps.len(), 2);
        assert!(eps.iter().all(|e| Path::new(&e.file_path).starts_with(&season_dir)));
    }

    #[test]
    fn destination_exists_still_organizes_in_place() {
        let dir = tempdir().unwrap();
        // Collision target already present (e.g. another season of the same series).
        let existing = dir.path().join("Solitary Gourmet (2012)");
        std::fs::create_dir_all(&existing).unwrap();
        let release = dir.path().join("Solitary Gourmet S11 WEB-DL");
        std::fs::create_dir_all(&release).unwrap();
        let ep1 = release.join("S11E01.mkv");
        std::fs::write(&ep1, b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();
        let item = MediaItem::new_show(
            MediaType::TvShow,
            "Solitary Gourmet",
            Some(2012),
            release.display().to_string(),
            library.id.clone(),
            ScrapedStatus::Scraped,
        );
        db.insert_media_items(&[item.clone()]).unwrap();
        db.insert_show_episodes(
            &item.id,
            &[ScannedEpisode {
                season: 11,
                episode: 1,
                file_path: ep1.display().to_string(),
                title: "Ep1".into(),
            }],
        )
        .unwrap();

        rename_after_scrape_with_options(&db, &item, &RenameTemplates::default(), true).unwrap();

        // Show folder rename skipped due to collision, but Season 11 + template rename still run.
        assert!(release.join("Season 11").is_dir() || existing.join("Season 11").is_dir());
        let eps = db.fetch_episodes(&format!("{}_S11", item.id)).unwrap();
        assert_eq!(eps.len(), 1);
        assert!(
            Path::new(&eps[0].file_path).components().any(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("Season 11")
            }),
            "episode should live under Season 11, got {}",
            eps[0].file_path
        );
    }

    #[test]
    fn consolidates_duplicate_tmdb_season_pack_into_canonical() {
        let dir = tempdir().unwrap();
        let main = dir.path().join("孤独的美食家 (2012)");
        let pack = dir.path().join("孤独的美食家.第十一季.WEB-DL.1080p");
        std::fs::create_dir_all(main.join("Season 01")).unwrap();
        std::fs::create_dir_all(&pack).unwrap();
        std::fs::write(main.join("Season 01/S01E01.mkv"), b"x").unwrap();
        let s11 = pack.join("S11E01.mkv");
        std::fs::write(&s11, b"x").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();

        let canonical = MediaItem::new_show(
            MediaType::TvShow,
            "孤独的美食家",
            Some(2012),
            main.display().to_string(),
            library.id.clone(),
            ScrapedStatus::Scraped,
        );
        let duplicate = MediaItem::new_show(
            MediaType::TvShow,
            "孤独的美食家",
            Some(2012),
            pack.display().to_string(),
            library.id.clone(),
            ScrapedStatus::Scraped,
        );
        db.insert_media_items(&[canonical.clone(), duplicate.clone()])
            .unwrap();
        db.insert_show_episodes(
            &canonical.id,
            &[ScannedEpisode {
                season: 1,
                episode: 1,
                file_path: main.join("Season 01/S01E01.mkv").display().to_string(),
                title: "Ep1".into(),
            }],
        )
        .unwrap();
        db.insert_show_episodes(
            &duplicate.id,
            &[ScannedEpisode {
                season: 11,
                episode: 1,
                file_path: s11.display().to_string(),
                title: "Ep1".into(),
            }],
        )
        .unwrap();

        let n = consolidate_library_duplicate_shows(&db, &library.id, &RenameTemplates::default())
            .unwrap();
        assert_eq!(n, 1);
        assert!(db.get_media_item(&duplicate.id).unwrap().is_none());
        let seasons = db.fetch_seasons(&canonical.id).unwrap();
        let nums: std::collections::HashSet<_> =
            seasons.iter().map(|s| s.season_number).collect();
        assert!(nums.contains(&1) && nums.contains(&11));
        assert!(main.join("Season 11").is_dir());
        assert!(
            !pack.exists(),
            "merged source folder should be removed after consolidate"
        );
    }

    #[test]
    fn organizes_moves_thumb_and_subtitle_companions() {
        let dir = tempdir().unwrap();
        let show = dir.path().join("Andor (2022)");
        std::fs::create_dir_all(&show).unwrap();
        let ep1 = show.join("Andor.S01E01.mkv");
        std::fs::write(&ep1, b"x").unwrap();
        std::fs::write(show.join("Andor.S01E01-thumb.jpg"), b"img").unwrap();
        std::fs::write(show.join("Andor.S01E01.zh.srt"), b"sub").unwrap();
        std::fs::write(show.join("poster.jpg"), b"poster").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();
        let item = MediaItem::new_show(
            MediaType::TvShow,
            "Andor",
            Some(2022),
            show.display().to_string(),
            library.id.clone(),
            ScrapedStatus::Scraped,
        );
        db.insert_media_items(&[item.clone()]).unwrap();
        db.insert_show_episodes(
            &item.id,
            &[ScannedEpisode {
                season: 1,
                episode: 1,
                file_path: ep1.display().to_string(),
                title: "Ep1".into(),
            }],
        )
        .unwrap();
        let season_id = format!("{}_S1", item.id);
        let mut eps = db.fetch_episodes(&season_id).unwrap();
        eps[0].still_path = Some("Andor.S01E01-thumb.jpg".into());
        db.upsert_episode(&eps[0]).unwrap();

        organize_season_folders(&db, &item, &RenameTemplates::default()).unwrap();

        let season_dir = show.join("Season 01");
        assert!(season_dir.is_dir());
        assert!(show.join("poster.jpg").is_file());
        assert!(!show.join("Andor.S01E01-thumb.jpg").exists());
        assert!(!show.join("Andor.S01E01.zh.srt").exists());

        let eps = db.fetch_episodes(&season_id).unwrap();
        let new_path = PathBuf::from(&eps[0].file_path);
        assert!(new_path.starts_with(&season_dir));
        assert!(new_path.is_file());
        let new_stem = new_path.file_stem().unwrap().to_str().unwrap();
        assert!(season_dir.join(format!("{new_stem}-thumb.jpg")).is_file());
        assert!(season_dir.join(format!("{new_stem}.zh.srt")).is_file());
        assert_eq!(
            eps[0].still_path.as_deref(),
            Some(format!("Season 01/{new_stem}-thumb.jpg").as_str())
        );
    }

    #[test]
    fn rename_updates_thumb_companion_stem() {
        let dir = tempdir().unwrap();
        let show = dir.path().join("Andor (2022)");
        let season = show.join("Season 01");
        std::fs::create_dir_all(&season).unwrap();
        let ep1 = season.join("Andor.S01E01.mkv");
        std::fs::write(&ep1, b"x").unwrap();
        std::fs::write(season.join("Andor.S01E01-thumb.jpg"), b"img").unwrap();

        let db = AppDatabase::open_in_memory().unwrap();
        let library = Library::new("TV", dir.path().display().to_string(), MediaType::TvShow);
        db.insert_library(&library).unwrap();
        let item = MediaItem::new_show(
            MediaType::TvShow,
            "Andor",
            Some(2022),
            show.display().to_string(),
            library.id.clone(),
            ScrapedStatus::Scraped,
        );
        db.insert_media_items(&[item.clone()]).unwrap();
        db.insert_show_episodes(
            &item.id,
            &[ScannedEpisode {
                season: 1,
                episode: 1,
                file_path: ep1.display().to_string(),
                title: "Kassa".into(),
            }],
        )
        .unwrap();
        let season_id = format!("{}_S1", item.id);
        let mut eps = db.fetch_episodes(&season_id).unwrap();
        eps[0].still_path = Some("Season 01/Andor.S01E01-thumb.jpg".into());
        db.upsert_episode(&eps[0]).unwrap();

        rename_after_scrape_with_options(&db, &item, &RenameTemplates::default(), false).unwrap();

        let eps = db.fetch_episodes(&season_id).unwrap();
        let new_path = PathBuf::from(&eps[0].file_path);
        assert!(new_path.exists());
        let new_stem = new_path.file_stem().unwrap().to_str().unwrap();
        assert_ne!(new_stem, "Andor.S01E01");
        assert!(season.join(format!("{new_stem}-thumb.jpg")).is_file());
        assert!(!season.join("Andor.S01E01-thumb.jpg").exists());
        assert_eq!(
            eps[0].still_path.as_deref(),
            Some(format!("Season 01/{new_stem}-thumb.jpg").as_str())
        );
    }
}
