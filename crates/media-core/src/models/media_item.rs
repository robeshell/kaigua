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

impl MediaItem {
    pub fn new_movie(
        title: impl Into<String>,
        year: Option<i32>,
        folder_path: impl Into<String>,
        file_path: impl Into<String>,
        library_id: impl Into<String>,
        status: ScrapedStatus,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            media_type: MediaType::Movie,
            title: title.into(),
            original_title: None,
            year,
            folder_path: folder_path.into(),
            file_path: file_path.into(),
            bookmark_data: None,
            status,
            scrape_issue: None,
            library_id: library_id.into(),
            added_at: Utc::now(),
        }
    }

    pub fn new_show(
        media_type: MediaType,
        title: impl Into<String>,
        year: Option<i32>,
        show_root: impl Into<String>,
        library_id: impl Into<String>,
        status: ScrapedStatus,
    ) -> Self {
        let root = show_root.into();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            media_type,
            title: title.into(),
            original_title: None,
            year,
            folder_path: root.clone(),
            file_path: root,
            bookmark_data: None,
            status,
            scrape_issue: None,
            library_id: library_id.into(),
            added_at: Utc::now(),
        }
    }
}
