use media_core::{MediaItem, MediaType};
use reqwest::Client;

use crate::bangumi::BangumiScraper;
use crate::matching::auto_accepted_result;
use crate::tmdb::TmdbScraper;
use crate::types::{ScrapedMetadata, SearchResult};

#[derive(Debug, Clone, Default)]
pub struct ScraperKeys {
    pub tmdb: String,
    pub bangumi: String,
}

#[derive(Debug, Clone)]
pub enum MatchOutcome {
    Matched(ScrapedMetadata),
    Unmatched { candidates: Vec<SearchResult> },
    Failed(String),
}

#[derive(Clone)]
pub struct ScraperCoordinator {
    tmdb: TmdbScraper,
    bangumi: BangumiScraper,
}

impl ScraperCoordinator {
    pub fn new(keys: ScraperKeys) -> Self {
        let client = Client::builder()
            .user_agent("kaigua/0.1.0")
            .build()
            .expect("http client");
        Self {
            tmdb: TmdbScraper::new(client.clone(), keys.tmdb),
            bangumi: BangumiScraper::new(client, keys.bangumi),
        }
    }

    pub async fn search_manual(
        &self,
        query: &str,
        media_type: MediaType,
        language: &str,
    ) -> Result<Vec<SearchResult>, String> {
        let mut all = Vec::new();
        for scraper in self.ordered(media_type) {
            match scraper {
                Source::Bangumi => match self.bangumi.search(query, media_type, language).await {
                    Ok(mut rows) => all.append(&mut rows),
                    Err(err) if err == "rateLimited" => continue,
                    Err(_) => continue,
                },
                Source::Tmdb => match self.tmdb.search(query, media_type, language).await {
                    Ok(mut rows) => all.append(&mut rows),
                    Err(err) if err == "rateLimited" => continue,
                    Err(_) => continue,
                },
            }
        }
        all.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(all)
    }

    pub async fn match_item(
        &self,
        item: &MediaItem,
        language: &str,
    ) -> MatchOutcome {
        let queries = build_queries(item);
        let mut best_candidates = Vec::new();
        for query in queries {
            for scraper in self.ordered(item.media_type) {
                let results = match scraper {
                    Source::Bangumi => {
                        self.bangumi
                            .search(&query, item.media_type, language)
                            .await
                    }
                    Source::Tmdb => self.tmdb.search(&query, item.media_type, language).await,
                };
                let results = match results {
                    Ok(rows) => rows,
                    Err(err) if err == "rateLimited" => continue,
                    Err(_) => continue,
                };
                if results.is_empty() {
                    continue;
                }
                best_candidates = results.clone();
                if let Some(accepted) =
                    auto_accepted_result(&item.title, item.year, item.media_type, &results)
                {
                    return match self
                        .fetch_by_source(&accepted.source_id, item.media_type, language)
                        .await
                    {
                        Ok(meta) => MatchOutcome::Matched(meta),
                        Err(err) => MatchOutcome::Failed(err),
                    };
                }
            }
        }
        if best_candidates.is_empty() {
            MatchOutcome::Failed("noResults".into())
        } else {
            MatchOutcome::Unmatched {
                candidates: best_candidates,
            }
        }
    }

    pub async fn fetch_by_source(
        &self,
        source_id: &str,
        media_type: MediaType,
        language: &str,
    ) -> Result<ScrapedMetadata, String> {
        if source_id.starts_with("bangumi:") {
            self.bangumi
                .fetch_metadata(source_id, media_type, language)
                .await
        } else if source_id.starts_with("tmdb:") {
            self.tmdb
                .fetch_metadata(source_id, media_type, language)
                .await
        } else {
            Err(format!("unknown source: {source_id}"))
        }
    }

    pub async fn fetch_artwork_urls(
        &self,
        source_id: &str,
        media_type: MediaType,
    ) -> Result<crate::types::ArtworkUrls, String> {
        if source_id.starts_with("bangumi:") {
            self.bangumi.fetch_artwork(source_id, media_type).await
        } else if source_id.starts_with("tmdb:") {
            self.tmdb.fetch_artwork(source_id, media_type).await
        } else {
            Err(format!("unknown source: {source_id}"))
        }
    }

    fn ordered(&self, media_type: MediaType) -> Vec<Source> {
        match media_type {
            MediaType::Anime => vec![Source::Bangumi, Source::Tmdb],
            MediaType::Movie | MediaType::TvShow => vec![Source::Tmdb],
        }
    }
}

enum Source {
    Tmdb,
    Bangumi,
}

fn build_queries(item: &MediaItem) -> Vec<String> {
    let mut queries = vec![item.title.clone()];
    if let Some(original) = &item.original_title {
        if !original.is_empty() && original != &item.title {
            queries.push(original.clone());
        }
    }
    queries
}
