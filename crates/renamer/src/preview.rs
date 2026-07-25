//! Rename preview (RENAME-R-09). Rules apply to stem only; extension is preserved.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::rules::RulePipeline;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub id: String,
    pub original_name: String,
    pub path: String,
}

impl FileEntry {
    pub fn new(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let original_name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        Self {
            id: Uuid::new_v4().to_string(),
            original_name,
            path: path.to_string_lossy().into_owned(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreviewResult {
    pub id: String,
    pub original_name: String,
    pub new_name: String,
    pub path: String,
    pub has_conflict: bool,
    pub has_invalid_chars: bool,
}

impl PreviewResult {
    pub fn is_executable(&self) -> bool {
        !self.has_conflict
            && !self.has_invalid_chars
            && self.original_name != self.new_name
            && !self.new_name.is_empty()
    }

    pub fn destination_path(&self) -> PathBuf {
        Path::new(&self.path)
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(&self.new_name)
    }
}

pub fn preview(files: &[FileEntry], pipeline: &RulePipeline) -> Vec<PreviewResult> {
    let mut results: Vec<PreviewResult> = files
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let (stem, ext) = split_extension(&file.original_name);
            let new_stem = pipeline.apply(stem, index);
            let new_name = if ext.is_empty() {
                new_stem.clone()
            } else {
                format!("{new_stem}.{ext}")
            };
            let has_invalid_chars = new_stem.chars().any(|c| matches!(c, '/' | ':' | '\0'));
            PreviewResult {
                id: file.id.clone(),
                original_name: file.original_name.clone(),
                new_name,
                path: file.path.clone(),
                has_conflict: false,
                has_invalid_chars,
            }
        })
        .collect();

    let mut counts: HashMap<String, usize> = HashMap::new();
    for r in &results {
        *counts.entry(r.new_name.clone()).or_default() += 1;
    }
    for r in &mut results {
        if counts.get(&r.new_name).copied().unwrap_or(0) > 1 {
            r.has_conflict = true;
        }
    }
    results
}

fn split_extension(filename: &str) -> (&str, &str) {
    match filename.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && !ext.contains('/') => {
            (stem, ext)
        }
        _ => (filename, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{AnyRenameRule, RulePipeline, TextReplace};

    #[test]
    fn applies_to_stem_only() {
        let pipeline = RulePipeline::new(vec![AnyRenameRule::TextReplace(TextReplace::new(
            "old", "new",
        ))]);
        let files = [FileEntry {
            id: "1".into(),
            original_name: "old.file.mkv".into(),
            path: "/tmp/old.file.mkv".into(),
        }];
        let out = preview(&files, &pipeline);
        assert_eq!(out[0].new_name, "new.file.mkv");
    }

    #[test]
    fn marks_conflicts() {
        let pipeline = RulePipeline::new(vec![AnyRenameRule::TextReplace(TextReplace::new(
            "a", "x",
        ))]);
        let files = [
            FileEntry {
                id: "1".into(),
                original_name: "a.mkv".into(),
                path: "/tmp/a.mkv".into(),
            },
            FileEntry {
                id: "2".into(),
                original_name: "a.txt".into(),
                path: "/tmp/a.txt".into(),
            },
        ];
        // Both become x.* — different names, no conflict.
        let out = preview(&files, &pipeline);
        assert!(!out[0].has_conflict);
        assert!(!out[1].has_conflict);

        let pipeline = RulePipeline::new(vec![AnyRenameRule::TextReplace(TextReplace::new(
            "one", "same",
        ))]);
        let files = [
            FileEntry {
                id: "1".into(),
                original_name: "one.mkv".into(),
                path: "/tmp/one.mkv".into(),
            },
            FileEntry {
                id: "2".into(),
                original_name: "one.mkv".into(),
                path: "/tmp/dir/one.mkv".into(),
            },
        ];
        // Same newName "same.mkv" → conflict even across dirs (Swift counts by newName only).
        let out = preview(&files, &pipeline);
        assert!(out.iter().all(|r| r.has_conflict));
    }

    #[test]
    fn marks_invalid_chars() {
        let pipeline = RulePipeline::new(vec![AnyRenameRule::TextReplace(TextReplace::new(
            "a", "a/b",
        ))]);
        let files = [FileEntry {
            id: "1".into(),
            original_name: "a.mkv".into(),
            path: "/tmp/a.mkv".into(),
        }];
        let out = preview(&files, &pipeline);
        assert!(out[0].has_invalid_chars);
        assert!(!out[0].is_executable());
    }
}
