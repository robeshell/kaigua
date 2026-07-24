//! Port of Swift `ThumbnailCache` disk tier (AppUI/Helpers/ThumbnailCache.swift).
//! Memory/preload land later (CACHE-02/03); M1 needs disk JPEG cache (CACHE-01).

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use image::imageops::FilterType;
use image::ImageReader;
use md5::{Digest, Md5};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ThumbnailError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("image error: {0}")]
    Image(String),
    #[error("source missing: {0}")]
    Missing(PathBuf),
}

pub struct ThumbnailCache {
    dir: PathBuf,
}

impl ThumbnailCache {
    pub fn open_default() -> Result<Self, ThumbnailError> {
        let caches = dirs::cache_dir().ok_or_else(|| {
            ThumbnailError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no cache directory",
            ))
        })?;
        Self::open(caches.join("kaigua").join("thumbnails"))
    }

    pub fn open(dir: impl Into<PathBuf>) -> Result<Self, ThumbnailError> {
        let dir = dir.into();
        fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    /// Pure string join — mirrors Swift `(folderPath as NSString).appendingPathComponent(posterPath)`.
    pub fn join_poster_path(folder_path: &str, poster_path: &str) -> PathBuf {
        Path::new(folder_path).join(poster_path)
    }

    /// Cache key without mtime: `path_WxH` (Swift `ThumbnailCache.key`).
    pub fn key(path: &str, width: u32, height: u32) -> String {
        format!("{path}_{width}x{height}")
    }

    /// Ensure a disk JPEG exists for `source` at target size; return cache file path.
    /// Disk key includes mtime like Swift (`key_mtime`) for poster replacement invalidation.
    pub fn ensure(
        &self,
        source: &Path,
        width: u32,
        height: u32,
    ) -> Result<PathBuf, ThumbnailError> {
        if !source.is_file() {
            return Err(ThumbnailError::Missing(source.to_path_buf()));
        }
        let path_str = source.to_string_lossy();
        let base_key = Self::key(&path_str, width, height);
        let mtime = mtime_secs(source);
        let disk_key = format!("{base_key}_{mtime}");
        let cache_path = self.file_url_for(&disk_key);
        if cache_path.is_file() {
            return Ok(cache_path);
        }

        let jpeg = downsample_to_jpeg(source, width, height)?;
        let tmp = cache_path.with_extension("jpg.tmp");
        fs::write(&tmp, &jpeg)?;
        fs::rename(&tmp, &cache_path)?;
        Ok(cache_path)
    }

    fn file_url_for(&self, key: &str) -> PathBuf {
        let mut hasher = Md5::new();
        hasher.update(key.as_bytes());
        let hex = format!("{:x}", hasher.finalize());
        self.dir.join(format!("{hex}.jpg"))
    }
}

fn mtime_secs(path: &Path) -> u64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn downsample_to_jpeg(path: &Path, width: u32, height: u32) -> Result<Vec<u8>, ThumbnailError> {
    // Swift uses maxDimension = max(w,h) * 2 for @2x retina.
    let max_dim = width.max(height).saturating_mul(2).max(1);
    let reader = ImageReader::open(path)
        .map_err(|e| ThumbnailError::Image(e.to_string()))?
        .with_guessed_format()
        .map_err(|e| ThumbnailError::Image(e.to_string()))?;
    let img = reader
        .decode()
        .map_err(|e| ThumbnailError::Image(e.to_string()))?;
    let resized = img.resize(max_dim, max_dim, FilterType::Triangle);
    let mut out = Cursor::new(Vec::new());
    resized
        .to_rgb8()
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .map_err(|e| ThumbnailError::Image(e.to_string()))?;
    Ok(out.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use tempfile::tempdir;

    #[test]
    fn ensure_writes_and_reuses_disk_cache() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("poster.png");
        let img = RgbImage::from_pixel(200, 300, Rgb([10, 20, 30]));
        img.save(&src).unwrap();

        let cache = ThumbnailCache::open(dir.path().join("thumbs")).unwrap();
        let first = cache.ensure(&src, 32, 46).unwrap();
        assert!(first.is_file());
        let second = cache.ensure(&src, 32, 46).unwrap();
        assert_eq!(first, second);
    }
}
