//! Minimal Kodi NFO writer (NFO-02/03/04).

use std::fs;
use std::path::Path;

use crate::models::{MediaItem, MediaMetadata, MediaType};

pub fn write_kodi_nfo(item: &MediaItem, metadata: &MediaMetadata) -> Result<(), String> {
    let folder = Path::new(&item.folder_path);
    fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let path = match item.media_type {
        MediaType::Movie => {
            let name = folder
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("movie");
            folder.join(format!("{name}.nfo"))
        }
        MediaType::TvShow | MediaType::Anime => folder.join("tvshow.nfo"),
    };
    let xml = render_kodi(item, metadata);
    fs::write(path, xml).map_err(|e| e.to_string())
}

fn render_kodi(item: &MediaItem, metadata: &MediaMetadata) -> String {
    let root = match item.media_type {
        MediaType::Movie => "movie",
        MediaType::TvShow | MediaType::Anime => "tvshow",
    };
    let mut out = String::new();
    out.push_str(&format!("<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n<{root}>\n"));
    out.push_str(&format!("  <title>{}</title>\n", xml_escape(&item.title)));
    if let Some(year) = item.year {
        out.push_str(&format!("  <year>{year}</year>\n"));
    }
    if let Some(plot) = &metadata.overview {
        out.push_str(&format!("  <plot>{}</plot>\n", xml_escape(plot)));
    }
    if let Some(rating) = metadata.rating {
        out.push_str(&format!("  <rating>{rating:.1}</rating>\n"));
    }
    for genre in &metadata.genres {
        out.push_str(&format!("  <genre>{}</genre>\n", xml_escape(genre)));
    }
    if let Some(id) = unique_id(metadata) {
        out.push_str(&format!(
            "  <uniqueid type=\"{}\" default=\"true\">{}</uniqueid>\n",
            id.0, id.1
        ));
    }
    out.push_str(&format!("</{root}>\n"));
    out
}

fn unique_id(metadata: &MediaMetadata) -> Option<(&'static str, String)> {
    if let Some(id) = &metadata.tmdb_id {
        return Some(("tmdb", id.clone()));
    }
    if let Some(id) = &metadata.bangumi_id {
        return Some(("bangumi", id.clone()));
    }
    if let Some(id) = &metadata.imdb_id {
        return Some(("imdb", id.clone()));
    }
    None
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
