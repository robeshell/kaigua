use std::path::{Path, PathBuf};

use reqwest::Client;

use crate::types::ArtworkUrls;

pub async fn download_artwork(
    client: &Client,
    folder: &Path,
    urls: &ArtworkUrls,
) -> Result<DownloadedArtwork, String> {
    let mut out = DownloadedArtwork::default();
    if let Some(url) = &urls.poster_url {
        out.poster_path = Some(download_one(client, folder, "poster.jpg", url).await?);
    }
    if let Some(url) = &urls.fanart_url {
        out.fanart_path = Some(download_one(client, folder, "fanart.jpg", url).await?);
    }
    if let Some(url) = &urls.banner_url {
        out.banner_path = Some(download_one(client, folder, "banner.jpg", url).await?);
    }
    Ok(out)
}

async fn download_one(
    client: &Client,
    folder: &Path,
    file_name: &str,
    url: &str,
) -> Result<String, String> {
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = folder.join(file_name);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(file_name.to_string())
}

#[derive(Debug, Clone, Default)]
pub struct DownloadedArtwork {
    pub poster_path: Option<String>,
    pub fanart_path: Option<String>,
    pub banner_path: Option<String>,
}

pub fn season_poster_name(season: i32) -> String {
    format!("season{season}-poster.jpg")
}

pub async fn download_to_name(
    client: &Client,
    folder: &Path,
    file_name: &str,
    url: &str,
) -> Result<PathBuf, String> {
    download_one(client, folder, file_name, url).await?;
    Ok(folder.join(file_name))
}
