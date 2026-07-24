use std::collections::HashSet;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::models::{Library, MediaItem, ScrapedStatus};

use super::filename::FileNameParser;

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "avi", "m4v", "mov", "wmv", "flv", "ts", "m2ts", "iso",
];

#[derive(Debug, Clone, Default)]
pub struct ScanProgress {
    pub discovered_count: u32,
    pub current_path: String,
    pub current_name: String,
}

#[derive(Debug, Clone, Default)]
pub struct MovieScanResult {
    pub new_items: Vec<MediaItem>,
}

pub fn scan_movies(
    library: &Library,
    existing_file_paths: &HashSet<String>,
    excluded_folders: &HashSet<String>,
    mut on_progress: impl FnMut(ScanProgress),
) -> Result<MovieScanResult, std::io::Error> {
    let root = PathBuf::from(&library.root_path);
    if !root.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("library root not found: {}", library.root_path),
        ));
    }

    let mut items = Vec::new();
    let mut discovered = 0u32;

    let walker = WalkDir::new(&root).follow_links(false).into_iter().filter_entry(|entry| {
        // Never skip the library root itself (temp paths may start with '.').
        if entry.depth() == 0 {
            return true;
        }
        let name = entry.file_name().to_string_lossy();
        if name.starts_with('.') {
            return false;
        }
        if entry.file_type().is_dir() {
            !excluded_folders.contains(&name.to_ascii_lowercase())
        } else {
            true
        }
    });

    for entry in walker {
        let entry = entry?;
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if !MEDIA_EXTENSIONS.contains(&ext.as_str()) {
            continue;
        }

        discovered += 1;
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();
        on_progress(ScanProgress {
            discovered_count: discovered,
            current_path: path.display().to_string(),
            current_name: file_name.clone(),
        });

        let absolute = canonicalize_lossy(path);
        if existing_file_paths.contains(&absolute) {
            continue;
        }

        let folder = path
            .parent()
            .map(canonicalize_lossy)
            .unwrap_or_else(|| absolute.clone());
        let parsed = FileNameParser::parse(&file_name);
        let nfo_path = path.with_extension("nfo");
        let status = if nfo_path.is_file() {
            ScrapedStatus::Scraped
        } else {
            ScrapedStatus::Unscraped
        };

        items.push(MediaItem::new_movie(
            parsed.title,
            parsed.year,
            folder,
            absolute,
            library.id.clone(),
            status,
        ));
    }

    Ok(MovieScanResult { new_items: items })
}

fn canonicalize_lossy(path: &Path) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MediaType;
    use tempfile::tempdir;

    #[test]
    fn scans_new_movie_files() {
        let dir = tempdir().unwrap();
        let movie_dir = dir.path().join("Oppenheimer (2023)");
        std::fs::create_dir_all(&movie_dir).unwrap();
        std::fs::write(movie_dir.join("Oppenheimer.2023.mkv"), b"x").unwrap();
        std::fs::write(movie_dir.join("ignore.txt"), b"x").unwrap();

        let library = Library::new("Movies", dir.path().display().to_string(), MediaType::Movie);
        let result = scan_movies(&library, &HashSet::new(), &HashSet::new(), |_| {}).unwrap();
        assert_eq!(result.new_items.len(), 1);
        assert_eq!(result.new_items[0].title, "Oppenheimer");
        assert_eq!(result.new_items[0].year, Some(2023));
    }
}
