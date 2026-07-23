use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{MediaType, ScrapedStatus};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub id: String,
    pub media_type: MediaType,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i32>,
    pub folder_path: String,
    pub file_path: String,
    pub bookmark_data: Option<Vec<u8>>,
    pub status: ScrapedStatus,
    pub scrape_issue: Option<String>,
    pub library_id: String,
    pub added_at: DateTime<Utc>,
}
