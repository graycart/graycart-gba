//! Recent ROM entries with friendly cartridge titles.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentRom {
    pub path: PathBuf,
    pub title: String,
}

impl RecentRom {
    pub fn new(path: PathBuf, title: impl Into<String>) -> Self {
        Self {
            path,
            title: title.into(),
        }
    }

    pub fn menu_label(&self) -> String {
        let name = self.title.trim();
        if name.is_empty() {
            self.path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("ROM")
                .to_string()
        } else {
            name.to_string()
        }
    }
}
