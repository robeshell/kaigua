use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::CastMember;

/// Aligned to Swift `MediaMetadata` + SQLite `media_metadata` columns.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadata {
    pub media_item_id: String,
    pub overview: Option<String>,
    pub outline: Option<String>,
    pub tagline: Option<String>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub rating: Option<f64>,
    pub rating_votes: Option<i32>,
    pub content_rating: Option<String>,
    pub director: Option<String>,
    pub writer: Option<String>,
    pub credits: Vec<CastMember>,
    pub studio: Option<String>,
    pub country: Option<String>,
    pub language: Option<String>,
    pub premiered: Option<String>,
    pub end_date: Option<String>,
    pub runtime: Option<i32>,
    pub show_status: Option<String>,
    pub collection_name: Option<String>,
    pub collection_id: Option<String>,
    pub source_id: String,
    pub imdb_id: Option<String>,
    pub tmdb_id: Option<String>,
    pub tvdb_id: Option<String>,
    pub bangumi_id: Option<String>,
    pub poster_path: Option<String>,
    pub fanart_path: Option<String>,
    pub banner_path: Option<String>,
    pub logo_path: Option<String>,
    pub thumb_path: Option<String>,
    pub video_codec: Option<String>,
    pub video_resolution: Option<String>,
    pub audio_codec: Option<String>,
    pub audio_channels: Option<String>,
    pub trailer: Option<String>,
    pub scraped_at: DateTime<Utc>,
}

impl MediaMetadata {
    pub fn actor_names(&self) -> Vec<String> {
        self.credits
            .iter()
            .filter(|c| c.r#type.as_deref().is_none_or(|t| t == "Actor"))
            .map(|c| c.name.clone())
            .collect()
    }
}
