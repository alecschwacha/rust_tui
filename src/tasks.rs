use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Task {
    pub title: String,
    pub description: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Tasks {
    pub todo: Vec<Task>,
    pub done: Vec<Task>,
}

pub fn path() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".rust_tui_tasks.json")
}

impl Tasks {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            // A corrupt file errors out instead of being silently overwritten.
            Ok(s) => serde_json::from_str(&s).with_context(|| format!("bad JSON in {}", path.display())),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Write to a temp file then rename, so a crash mid-write can't truncate the task list.
    pub fn save(&self, path: &Path) -> Result<()> {
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        fs::rename(&tmp, path)?;
        Ok(())
    }
}
