use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use media_core::{AppDatabase, MediaItem, MediaMetadata, MediaType, ScrapedStatus, TvEpisode, TvSeason};
use reqwest::Client;

use crate::artwork::{download_artwork, download_to_name, season_poster_name};
use crate::coordinator::{MatchOutcome, ScraperCoordinator, ScraperKeys};
use crate::types::ScrapedMetadata;

#[derive(Debug, Clone)]
pub struct ScrapeOptions {
    pub language: String,
    pub concurrency: usize,
    pub keys: ScraperKeys,
    /// `kodi` | `emby` (NFO-05)
    pub nfo_format: String,
}

impl Default for ScrapeOptions {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            concurrency: 4,
            keys: ScraperKeys::default(),
            nfo_format: "kodi".into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ScrapeProgress {
    pub completed: u32,
    pub total: u32,
    pub current: String,
    pub stage_key: String,
}

/// Outcome of scraping one media item (auto-match path).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrapeItemOutcome {
    Matched,
    Unmatched,
    Failed,
}

#[derive(Debug, Clone, Default)]
pub struct ScrapeSummary {
    pub success_ids: Vec<String>,
    pub unmatched: u32,
    pub failed: u32,
}

impl ScrapeSummary {
    pub fn format_result(&self) -> String {
        format!(
            "success={} unmatched={} failed={}",
            self.success_ids.len(),
            self.unmatched,
            self.failed
        )
    }

    pub fn parse_result(s: &str) -> Option<(u32, u32, u32)> {
        let mut success = None;
        let mut unmatched = None;
        let mut failed = None;
        for part in s.split_whitespace() {
            if let Some(v) = part.strip_prefix("success=") {
                success = v.parse().ok();
            } else if let Some(v) = part.strip_prefix("unmatched=") {
                unmatched = v.parse().ok();
            } else if let Some(v) = part.strip_prefix("failed=") {
                failed = v.parse().ok();
            }
        }
        Some((success?, unmatched?, failed?))
    }
}

pub async fn scrape_library(
    db: Arc<AppDatabase>,
    library_id: &str,
    options: ScrapeOptions,
    mut on_progress: impl FnMut(ScrapeProgress) + Send,
) -> Result<ScrapeSummary, String> {
    let items = db
        .list_media_items(library_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|i| i.status == ScrapedStatus::Unscraped)
        .collect::<Vec<_>>();
    let total = items.len() as u32;
    let mut done = 0u32;
    let mut summary = ScrapeSummary::default();
    let coordinator = ScraperCoordinator::new(options.keys.clone());
    let client = crate::http::build_client();
    let sem = Arc::new(tokio::sync::Semaphore::new(options.concurrency.max(1)));
    let mut handles = Vec::new();
    for item in items {
        let permit = Arc::clone(&sem).acquire_owned().await.map_err(|e| e.to_string())?;
        let db = Arc::clone(&db);
        let coordinator = coordinator.clone();
        let client = client.clone();
        let language = options.language.clone();
        let nfo_format = options.nfo_format.clone();
        let item_id = item.id.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let title = item.title.clone();
            let result =
                scrape_item_inner(&db, &coordinator, &client, &item, &language, &nfo_format).await;
            (item_id, title, result)
        }));
    }
    for handle in handles {
        let (item_id, title, result) = handle.await.map_err(|e| e.to_string())?;
        done += 1;
        on_progress(ScrapeProgress {
            completed: done,
            total,
            current: title,
            stage_key: "matching".into(),
        });
        match result {
            Ok(ScrapeItemOutcome::Matched) => summary.success_ids.push(item_id),
            Ok(ScrapeItemOutcome::Unmatched) => summary.unmatched += 1,
            Ok(ScrapeItemOutcome::Failed) => summary.failed += 1,
            Err(e) => return Err(e),
        }
    }
    Ok(summary)
}

pub async fn scrape_item(
    db: &AppDatabase,
    item: &MediaItem,
    options: &ScrapeOptions,
) -> Result<ScrapeItemOutcome, String> {
    let coordinator = ScraperCoordinator::new(options.keys.clone());
    let client = crate::http::build_client();
    scrape_item_inner(
        db,
        &coordinator,
        &client,
        item,
        &options.language,
        &options.nfo_format,
    )
    .await
}

pub async fn apply_manual_match(
    db: &AppDatabase,
    item: &MediaItem,
    source_id: &str,
    options: &ScrapeOptions,
) -> Result<(), String> {
    let coordinator = ScraperCoordinator::new(options.keys.clone());
    let client = crate::http::build_client();
    let meta = coordinator
        .fetch_by_source(source_id, item.media_type, &options.language)
        .await
        .map_err(|e| crate::http::humanize_error(&e))?;
    persist_match(db, &client, item, meta, &options.nfo_format).await?;
    Ok(())
}

async fn scrape_item_inner(
    db: &AppDatabase,
    coordinator: &ScraperCoordinator,
    client: &Client,
    item: &MediaItem,
    language: &str,
    nfo_format: &str,
) -> Result<ScrapeItemOutcome, String> {
    match coordinator.match_item(item, language).await {
        MatchOutcome::Matched(meta) => {
            persist_match(db, client, item, meta, nfo_format).await?;
            Ok(ScrapeItemOutcome::Matched)
        }
        MatchOutcome::Unmatched { .. } => {
            db.update_status(&item.id, ScrapedStatus::Unmatched, None)
                .map_err(|e| e.to_string())?;
            Ok(ScrapeItemOutcome::Unmatched)
        }
        MatchOutcome::Failed(err) => {
            if err == "noResults" {
                db.update_status(&item.id, ScrapedStatus::Unmatched, Some(&err))
                    .map_err(|e| e.to_string())?;
                Ok(ScrapeItemOutcome::Unmatched)
            } else {
                db.update_status(&item.id, ScrapedStatus::Partial, Some(&err))
                    .map_err(|e| e.to_string())?;
                Ok(ScrapeItemOutcome::Failed)
            }
        }
    }
}

async fn persist_match(
    db: &AppDatabase,
    client: &Client,
    item: &MediaItem,
    scraped: ScrapedMetadata,
    nfo_format: &str,
) -> Result<(), String> {
    let folder = Path::new(&item.folder_path);
    let artwork = download_artwork(
        client,
        folder,
        &crate::types::ArtworkUrls {
            poster_url: scraped.poster_url.clone(),
            fanart_url: scraped.fanart_url.clone(),
            banner_url: scraped.banner_url.clone(),
        },
    )
    .await
    .unwrap_or_default();

    for season in &scraped.seasons {
        if let Some(url) = &season.poster_url {
            let name = season_poster_name(season.season_number);
            let _ = download_to_name(client, folder, &name, url).await;
        }
    }

    let metadata = MediaMetadata {
        media_item_id: item.id.clone(),
        overview: scraped.overview.clone(),
        outline: None,
        tagline: scraped.tagline.clone(),
        genres: scraped.genres.clone(),
        tags: scraped.tags.clone(),
        rating: scraped.rating,
        rating_votes: scraped.rating_votes,
        content_rating: scraped.content_rating.clone(),
        director: scraped.director.clone(),
        writer: scraped.writer.clone(),
        credits: scraped.credits.clone(),
        studio: scraped.studio.clone(),
        country: scraped.country.clone(),
        language: scraped.language.clone(),
        premiered: scraped.premiered.clone(),
        end_date: scraped.end_date.clone(),
        runtime: scraped.runtime,
        show_status: scraped.show_status.clone(),
        collection_name: scraped.collection_name.clone(),
        collection_id: scraped.collection_id.clone(),
        source_id: scraped.source_id.clone(),
        imdb_id: scraped.imdb_id.clone(),
        tmdb_id: scraped.tmdb_id.clone(),
        tvdb_id: scraped.tvdb_id.clone(),
        bangumi_id: scraped.bangumi_id.clone(),
        poster_path: artwork.poster_path.clone(),
        fanart_path: artwork.fanart_path.clone(),
        banner_path: artwork.banner_path.clone(),
        logo_path: None,
        thumb_path: None,
        video_codec: None,
        video_resolution: None,
        audio_codec: None,
        audio_channels: None,
        trailer: scraped.trailer.clone(),
        scraped_at: Utc::now(),
    };
    db.upsert_metadata(&metadata).map_err(|e| e.to_string())?;
    db.update_title(
        &item.id,
        &scraped.title,
        scraped.original_title.as_deref(),
    )
    .map_err(|e| e.to_string())?;
    if let Some(year) = scraped.year {
        db.update_year(&item.id, Some(year))
            .map_err(|e| e.to_string())?;
    }
    if matches!(item.media_type, MediaType::TvShow | MediaType::Anime) {
        merge_seasons(db, client, item, &scraped.seasons).await?;
    }
    let _ = media_core::nfo::write_nfo(item, &metadata, nfo_format);
    db.update_status(&item.id, ScrapedStatus::Scraped, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Refresh one season's metadata / episode info from TMDB (show must already be scraped).
pub async fn scrape_season(
    db: &AppDatabase,
    item: &MediaItem,
    season_number: i32,
    options: &ScrapeOptions,
) -> Result<(), String> {
    if !matches!(item.media_type, MediaType::TvShow | MediaType::Anime) {
        return Err("err.notTvShow".into());
    }
    let meta = db
        .fetch_metadata(&item.id)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "err.notScraped".to_string())?;
    let tmdb_id = meta
        .tmdb_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| {
            meta.source_id
                .strip_prefix("tmdb:")
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        })
        .ok_or_else(|| "err.noTmdbId".to_string())?;
    if options.keys.tmdb.trim().is_empty() {
        return Err("err.apiKey".into());
    }
    let client = crate::http::build_client();
    let tmdb = crate::tmdb::TmdbScraper::new(client.clone(), options.keys.tmdb.clone());
    let scraped = tmdb
        .fetch_season(&tmdb_id, season_number, &options.language)
        .await?;
    let folder = Path::new(&item.folder_path);
    if let Some(url) = &scraped.poster_url {
        let name = season_poster_name(scraped.season_number);
        let _ = download_to_name(&client, folder, &name, url).await;
    }
    merge_seasons(db, &client, item, &[scraped]).await
}

async fn merge_seasons(
    db: &AppDatabase,
    client: &Client,
    item: &MediaItem,
    scraped_seasons: &[crate::types::ScrapedSeason],
) -> Result<(), String> {
    let folder = Path::new(&item.folder_path);
    let existing_seasons = db.fetch_seasons(&item.id).map_err(|e| e.to_string())?;
    for scraped_season in scraped_seasons {
        let season_id = existing_seasons
            .iter()
            .find(|s| s.season_number == scraped_season.season_number)
            .map(|s| s.id.clone())
            .unwrap_or_else(|| format!("{}_S{}", item.id, scraped_season.season_number));
        let poster = scraped_season
            .poster_url
            .as_ref()
            .map(|_| season_poster_name(scraped_season.season_number));
        let season = TvSeason {
            id: season_id.clone(),
            media_item_id: item.id.clone(),
            season_number: scraped_season.season_number,
            title: scraped_season.title.clone(),
            overview: scraped_season.overview.clone(),
            poster_path: poster,
            air_date: scraped_season.air_date.clone(),
            episode_count: scraped_season.episode_count,
        };
        db.upsert_season(&season).map_err(|e| e.to_string())?;
        let existing_eps = db.fetch_episodes(&season_id).map_err(|e| e.to_string())?;
        for scraped_ep in &scraped_season.episodes {
            if let Some(existing) = existing_eps
                .iter()
                .find(|e| e.episode_number == scraped_ep.episode_number)
            {
                let mut still_path = existing.still_path.clone();
                let mut still_url = scraped_ep.still_url.clone().or_else(|| existing.still_url.clone());
                // SCRAPE-17: download episode still next to the media file.
                if let Some(url) = scraped_ep.still_url.as_ref() {
                    if !existing.file_path.is_empty() {
                        let ep_path = Path::new(&existing.file_path);
                        if let Some(stem) = ep_path.file_stem().and_then(|s| s.to_str()) {
                            let file_name = format!("{stem}-thumb.jpg");
                            let dest_dir = ep_path.parent().unwrap_or(folder);
                            if let Ok(abs) = download_to_name(client, dest_dir, &file_name, url).await
                            {
                                still_path = Some(relative_to_show(&abs, folder));
                                still_url = Some(url.clone());
                            }
                        }
                    }
                }
                let updated = TvEpisode {
                    title: scraped_ep.title.clone().or_else(|| existing.title.clone()),
                    overview: scraped_ep
                        .overview
                        .clone()
                        .or_else(|| existing.overview.clone()),
                    air_date: scraped_ep
                        .air_date
                        .clone()
                        .or_else(|| existing.air_date.clone()),
                    runtime: scraped_ep.runtime.or(existing.runtime),
                    rating: scraped_ep.rating.or(existing.rating),
                    director: scraped_ep
                        .director
                        .clone()
                        .or_else(|| existing.director.clone()),
                    writer: scraped_ep.writer.clone().or_else(|| existing.writer.clone()),
                    still_path,
                    still_url,
                    ..existing.clone()
                };
                db.upsert_episode(&updated).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}

fn relative_to_show(path: &Path, show_root: &Path) -> String {
    path.strip_prefix(show_root)
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}
