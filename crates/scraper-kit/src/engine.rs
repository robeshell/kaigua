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
}

impl Default for ScrapeOptions {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            concurrency: 4,
            keys: ScraperKeys::default(),
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

pub async fn scrape_library(
    db: Arc<AppDatabase>,
    library_id: &str,
    options: ScrapeOptions,
    mut on_progress: impl FnMut(ScrapeProgress) + Send,
) -> Result<u32, String> {
    let items = db
        .list_media_items(library_id)
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|i| i.status == ScrapedStatus::Unscraped)
        .collect::<Vec<_>>();
    let total = items.len() as u32;
    let mut done = 0u32;
    let coordinator = ScraperCoordinator::new(options.keys.clone());
    let client = Client::new();
    let sem = Arc::new(tokio::sync::Semaphore::new(options.concurrency.max(1)));
    let mut handles = Vec::new();
    for item in items {
        let permit = Arc::clone(&sem).acquire_owned().await.map_err(|e| e.to_string())?;
        let db = Arc::clone(&db);
        let coordinator = coordinator.clone();
        let client = client.clone();
        let language = options.language.clone();
        handles.push(tokio::spawn(async move {
            let _permit = permit;
            let title = item.title.clone();
            let result = scrape_item_inner(&db, &coordinator, &client, &item, &language).await;
            (title, result)
        }));
    }
    for handle in handles {
        let (title, result) = handle.await.map_err(|e| e.to_string())?;
        done += 1;
        on_progress(ScrapeProgress {
            completed: done,
            total,
            current: title,
            stage_key: "matching".into(),
        });
        result?;
    }
    Ok(done)
}

pub async fn scrape_item(
    db: &AppDatabase,
    item: &MediaItem,
    options: &ScrapeOptions,
) -> Result<(), String> {
    let coordinator = ScraperCoordinator::new(options.keys.clone());
    let client = Client::new();
    scrape_item_inner(db, &coordinator, &client, item, &options.language).await
}

pub async fn apply_manual_match(
    db: &AppDatabase,
    item: &MediaItem,
    source_id: &str,
    options: &ScrapeOptions,
) -> Result<(), String> {
    let coordinator = ScraperCoordinator::new(options.keys.clone());
    let client = Client::new();
    let meta = coordinator
        .fetch_by_source(source_id, item.media_type, &options.language)
        .await?;
    persist_match(db, &client, item, meta).await
}

async fn scrape_item_inner(
    db: &AppDatabase,
    coordinator: &ScraperCoordinator,
    client: &Client,
    item: &MediaItem,
    language: &str,
) -> Result<(), String> {
    match coordinator.match_item(item, language).await {
        MatchOutcome::Matched(meta) => persist_match(db, client, item, meta).await,
        MatchOutcome::Unmatched { .. } => {
            db.update_status(&item.id, ScrapedStatus::Unmatched, None)
                .map_err(|e| e.to_string())
        }
        MatchOutcome::Failed(err) => {
            if err == "noResults" {
                db.update_status(&item.id, ScrapedStatus::Unmatched, Some(&err))
                    .map_err(|e| e.to_string())
            } else {
                db.update_status(&item.id, ScrapedStatus::Partial, Some(&err))
                    .map_err(|e| e.to_string())
            }
        }
    }
}

async fn persist_match(
    db: &AppDatabase,
    client: &Client,
    item: &MediaItem,
    scraped: ScrapedMetadata,
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
        merge_seasons(db, item, &scraped)?;
    }
    let _ = media_core::nfo::write_kodi_nfo(item, &metadata);
    db.update_status(&item.id, ScrapedStatus::Scraped, None)
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn merge_seasons(
    db: &AppDatabase,
    item: &MediaItem,
    scraped: &ScrapedMetadata,
) -> Result<(), String> {
    let existing_seasons = db.fetch_seasons(&item.id).map_err(|e| e.to_string())?;
    for scraped_season in &scraped.seasons {
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
                let updated = TvEpisode {
                    title: scraped_ep.title.clone().or_else(|| existing.title.clone()),
                    overview: scraped_ep.overview.clone().or_else(|| existing.overview.clone()),
                    air_date: scraped_ep.air_date.clone().or_else(|| existing.air_date.clone()),
                    runtime: scraped_ep.runtime.or(existing.runtime),
                    rating: scraped_ep.rating.or(existing.rating),
                    director: scraped_ep.director.clone().or_else(|| existing.director.clone()),
                    writer: scraped_ep.writer.clone().or_else(|| existing.writer.clone()),
                    ..existing.clone()
                };
                db.upsert_episode(&updated).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(())
}
