use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::MediaType;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Library {
    pub id: String,
    pub name: String,
    pub root_path: String,
    /// Optional platform bookmark blob; unused on non-sandboxed desktop.
    pub bookmark_data: Option<Vec<u8>>,
    pub media_type: MediaType,
    pub added_at: DateTime<Utc>,
}

impl Library {
    pub fn new(name: impl Into<String>, root_path: impl Into<String>, media_type: MediaType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.into(),
            root_path: root_path.into(),
            bookmark_data: None,
            media_type,
            added_at: Utc::now(),
        }
    }
}
