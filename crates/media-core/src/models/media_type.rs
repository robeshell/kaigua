use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaType {
    Movie,
    #[serde(rename = "tvShow")]
    TvShow,
    Anime,
}

impl MediaType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::TvShow => "tvShow",
            Self::Anime => "anime",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "movie" => Some(Self::Movie),
            "tvShow" => Some(Self::TvShow),
            "anime" => Some(Self::Anime),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScrapedStatus {
    Unscraped,
    Scraped,
    Unmatched,
    Partial,
}

impl ScrapedStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unscraped => "unscraped",
            Self::Scraped => "scraped",
            Self::Unmatched => "unmatched",
            Self::Partial => "partial",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "unscraped" => Some(Self::Unscraped),
            "scraped" => Some(Self::Scraped),
            "unmatched" => Some(Self::Unmatched),
            "partial" => Some(Self::Partial),
            _ => None,
        }
    }
}
