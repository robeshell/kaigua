use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TvSeason {
    pub id: String,
    pub media_item_id: String,
    pub season_number: i32,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub air_date: Option<String>,
    pub episode_count: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TvEpisode {
    pub id: String,
    pub season_id: String,
    pub episode_number: i32,
    pub title: Option<String>,
    pub overview: Option<String>,
    pub air_date: Option<String>,
    pub still_path: Option<String>,
    pub still_url: Option<String>,
    pub file_path: String,
    pub runtime: Option<i32>,
    pub rating: Option<f64>,
    pub director: Option<String>,
    pub writer: Option<String>,
    pub guest_cast: Vec<crate::models::CastMember>,
    pub absolute_number: Option<i32>,
    pub finale_type: Option<String>,
    pub video_codec: Option<String>,
    pub video_resolution: Option<String>,
    pub audio_codec: Option<String>,
}
