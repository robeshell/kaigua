//! Scraper kit — TMDB + Bangumi matching and scrape pipeline (M2).

pub mod artwork;
pub mod bangumi;
pub mod coordinator;
pub mod engine;
pub mod matching;
pub mod tmdb;
pub mod types;

pub use coordinator::{MatchOutcome, ScraperCoordinator, ScraperKeys};
pub use engine::{
    apply_manual_match, scrape_item, scrape_library, ScrapeOptions, ScrapeProgress,
};
pub use matching::{auto_accepted_result, normalize_title, relevance_score};
pub use types::{ArtworkUrls, ScrapedEpisode, ScrapedMetadata, ScrapedSeason, SearchResult};

pub fn crate_name() -> &'static str {
    "scraper-kit"
}
