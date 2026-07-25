//! Rename execute + undo (RENAME-R-10/11).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use media_core::{CollisionPolicy, FilesystemService};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::preview::PreviewResult;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompletedRename {
    pub original_path: String,
    pub new_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RenameSnapshot {
    pub id: String,
    pub date: DateTime<Utc>,
    pub renames: Vec<CompletedRename>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExecuteError {
    #[error(transparent)]
    Filesystem(#[from] media_core::FilesystemError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("no undo snapshots")]
    NoSnapshots,
}

/// Execute renames from preview. Skips conflict / invalid / unchanged.
/// Saves an undo snapshot when anything moved.
pub fn execute(
    previews: &[PreviewResult],
    undo: &RenameUndoManager,
) -> Result<Vec<CompletedRename>, ExecuteError> {
    let fs = FilesystemService::new();
    let mut completed = Vec::new();
    for preview in previews.iter().filter(|p| p.is_executable()) {
        let src = PathBuf::from(&preview.path);
        let dst = preview.destination_path();
        if !src.is_file() && !src.is_dir() {
            continue;
        }
        if dst.exists() {
            continue;
        }
        fs.move_item(&src, &dst, CollisionPolicy::Fail)?;
        completed.push(CompletedRename {
            original_path: src.to_string_lossy().into_owned(),
            new_path: dst.to_string_lossy().into_owned(),
        });
    }
    if !completed.is_empty() {
        undo.save_snapshot(&completed)?;
    }
    Ok(completed)
}

pub struct RenameUndoManager {
    storage_dir: PathBuf,
    max_snapshots: usize,
}

impl RenameUndoManager {
    pub fn open(storage_dir: impl Into<PathBuf>) -> Result<Self, ExecuteError> {
        let storage_dir = storage_dir.into();
        fs::create_dir_all(&storage_dir)?;
        Ok(Self {
            storage_dir,
            max_snapshots: 10,
        })
    }

    pub fn open_default() -> Result<Self, ExecuteError> {
        let base = dirs::data_dir().ok_or_else(|| {
            ExecuteError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "no data directory",
            ))
        })?;
        Self::open(base.join("kaigua").join("rename_snapshots"))
    }

    pub fn save_snapshot(&self, renames: &[CompletedRename]) -> Result<(), ExecuteError> {
        let snapshot = RenameSnapshot {
            id: Uuid::new_v4().to_string(),
            date: Utc::now(),
            renames: renames.to_vec(),
        };
        let path = self.storage_dir.join(format!("{}.json", snapshot.id));
        let data = serde_json::to_vec_pretty(&snapshot)?;
        fs::write(path, data)?;
        self.trim()?;
        Ok(())
    }

    pub fn snapshots(&self) -> Result<Vec<RenameSnapshot>, ExecuteError> {
        let mut out = self.load_all()?;
        out.sort_by(|a, b| b.date.cmp(&a.date));
        Ok(out)
    }

    pub fn undo_last(&self) -> Result<usize, ExecuteError> {
        let mut all = self.load_all()?;
        all.sort_by(|a, b| b.date.cmp(&a.date));
        let Some(latest) = all.into_iter().next() else {
            return Err(ExecuteError::NoSnapshots);
        };
        let fs = FilesystemService::new();
        let mut n = 0usize;
        for rename in latest.renames.iter().rev() {
            let new_path = Path::new(&rename.new_path);
            let original = Path::new(&rename.original_path);
            if new_path.exists() {
                fs.move_item(new_path, original, CollisionPolicy::Fail)?;
                n += 1;
            }
        }
        let snap = self.storage_dir.join(format!("{}.json", latest.id));
        let _ = fs::remove_file(snap);
        Ok(n)
    }

    fn load_all(&self) -> Result<Vec<RenameSnapshot>, ExecuteError> {
        if !self.storage_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for entry in fs::read_dir(&self.storage_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let data = fs::read(&path)?;
            if let Ok(snap) = serde_json::from_slice::<RenameSnapshot>(&data) {
                out.push(snap);
            }
        }
        Ok(out)
    }

    fn trim(&self) -> Result<(), ExecuteError> {
        let mut all = self.load_all()?;
        if all.len() <= self.max_snapshots {
            return Ok(());
        }
        all.sort_by(|a, b| a.date.cmp(&b.date));
        let excess = all.len() - self.max_snapshots;
        for old in all.into_iter().take(excess) {
            let path = self.storage_dir.join(format!("{}.json", old.id));
            let _ = fs::remove_file(path);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preview::{preview, FileEntry};
    use crate::rules::{AnyRenameRule, RulePipeline, TextReplace};
    use tempfile::tempdir;

    #[test]
    fn execute_and_undo() {
        let dir = tempdir().unwrap();
        let src = dir.path().join("old_name.mkv");
        fs::write(&src, b"x").unwrap();

        let pipeline = RulePipeline::new(vec![AnyRenameRule::TextReplace(TextReplace::new(
            "old_name", "new_name",
        ))]);
        let files = [FileEntry::new(&src)];
        let previews = preview(&files, &pipeline);
        assert!(previews[0].is_executable());

        let undo = RenameUndoManager::open(dir.path().join("snaps")).unwrap();
        let done = execute(&previews, &undo).unwrap();
        assert_eq!(done.len(), 1);
        assert!(!src.exists());
        assert!(dir.path().join("new_name.mkv").is_file());

        let n = undo.undo_last().unwrap();
        assert_eq!(n, 1);
        assert!(src.is_file());
        assert!(!dir.path().join("new_name.mkv").exists());
    }
}
