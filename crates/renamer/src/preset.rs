//! Rename rule presets (RENAME-R-12). File-backed, aligned to Swift `PresetManager`.

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::rules::RulePipeline;

#[derive(Debug, thiserror::Error)]
pub enum PresetError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    #[error("invalid preset name")]
    InvalidName,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
struct PresetIndex {
    names: Vec<String>,
}

pub struct PresetManager {
    storage_dir: PathBuf,
}

impl PresetManager {
    pub fn open(storage_dir: impl Into<PathBuf>) -> Result<Self, PresetError> {
        let storage_dir = storage_dir.into();
        fs::create_dir_all(storage_dir.join("presets"))?;
        Ok(Self { storage_dir })
    }

    pub fn save(&self, name: &str, pipeline: &RulePipeline) -> Result<(), PresetError> {
        let name = validate_name(name)?;
        let path = self.preset_path(&name)?;
        let data = serde_json::to_vec_pretty(pipeline)?;
        fs::write(path, data)?;
        let mut index = self.load_index()?;
        if !index.names.iter().any(|n| n == &name) {
            index.names.push(name);
            self.save_index(&index)?;
        }
        Ok(())
    }

    pub fn load(&self, name: &str) -> Result<Option<RulePipeline>, PresetError> {
        let name = validate_name(name)?;
        let path = self.preset_path(&name)?;
        if !path.is_file() {
            return Ok(None);
        }
        let data = fs::read(path)?;
        Ok(Some(serde_json::from_slice(&data)?))
    }

    pub fn list_presets(&self) -> Result<Vec<String>, PresetError> {
        Ok(self.load_index()?.names)
    }

    pub fn delete(&self, name: &str) -> Result<(), PresetError> {
        let name = validate_name(name)?;
        let path = self.preset_path(&name)?;
        let _ = fs::remove_file(path);
        let mut index = self.load_index()?;
        index.names.retain(|n| n != &name);
        self.save_index(&index)?;
        Ok(())
    }

    pub fn auto_save(&self, pipeline: &RulePipeline) -> Result<(), PresetError> {
        let path = self.storage_dir.join("autosave.json");
        let data = serde_json::to_vec_pretty(pipeline)?;
        fs::write(path, data)?;
        Ok(())
    }

    pub fn auto_load(&self) -> Result<Option<RulePipeline>, PresetError> {
        let path = self.storage_dir.join("autosave.json");
        if !path.is_file() {
            return Ok(None);
        }
        let data = fs::read(path)?;
        Ok(Some(serde_json::from_slice(&data)?))
    }

    fn preset_path(&self, name: &str) -> Result<PathBuf, PresetError> {
        let file = format!("{}.json", sanitize_file_stem(name));
        Ok(self.storage_dir.join("presets").join(file))
    }

    fn index_path(&self) -> PathBuf {
        self.storage_dir.join("index.json")
    }

    fn load_index(&self) -> Result<PresetIndex, PresetError> {
        let path = self.index_path();
        if !path.is_file() {
            return Ok(PresetIndex::default());
        }
        let data = fs::read(path)?;
        Ok(serde_json::from_slice(&data)?)
    }

    fn save_index(&self, index: &PresetIndex) -> Result<(), PresetError> {
        let data = serde_json::to_vec_pretty(index)?;
        fs::write(self.index_path(), data)?;
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<String, PresetError> {
    let name = name.trim();
    if name.is_empty() || name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err(PresetError::InvalidName);
    }
    if name == "." || name == ".." || name.len() > 128 {
        return Err(PresetError::InvalidName);
    }
    Ok(name.to_string())
}

fn sanitize_file_stem(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{AnyRenameRule, CaseConversion, CaseMode, RulePipeline, TextReplace};
    use tempfile::tempdir;

    #[test]
    fn save_load_list_delete() {
        let dir = tempdir().unwrap();
        let mgr = PresetManager::open(dir.path()).unwrap();
        let pipeline = RulePipeline::new(vec![AnyRenameRule::TextReplace(TextReplace::new(
            "old", "new",
        ))]);
        mgr.save("test-preset", &pipeline).unwrap();
        let loaded = mgr.load("test-preset").unwrap().unwrap();
        assert_eq!(loaded.apply("old file", 0), "new file");
        assert!(mgr.list_presets().unwrap().contains(&"test-preset".into()));
        mgr.delete("test-preset").unwrap();
        assert!(!mgr.list_presets().unwrap().contains(&"test-preset".into()));
        assert!(mgr.load("test-preset").unwrap().is_none());
    }

    #[test]
    fn auto_save_and_load() {
        let dir = tempdir().unwrap();
        let mgr = PresetManager::open(dir.path()).unwrap();
        let pipeline = RulePipeline::new(vec![AnyRenameRule::CaseConversion(
            CaseConversion::new(CaseMode::Upper),
        )]);
        mgr.auto_save(&pipeline).unwrap();
        let loaded = mgr.auto_load().unwrap().unwrap();
        assert_eq!(loaded.apply("test", 0), "TEST");
    }

    #[test]
    fn rejects_bad_names() {
        let dir = tempdir().unwrap();
        let mgr = PresetManager::open(dir.path()).unwrap();
        let pipeline = RulePipeline::default();
        assert!(mgr.save("../x", &pipeline).is_err());
        assert!(mgr.save("", &pipeline).is_err());
    }
}
