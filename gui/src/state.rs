//! Pure application state & domain data.
//! Keep this file free of egui specifics when possible.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AppState {
    pub project_label: String,
    pub selected_file: Option<PathBuf>,
    pub selected_folder: Option<PathBuf>,
    pub target_language: String,
    pub preserve_honorifics: bool,
    pub dry_run: bool,
    pub is_processing: bool,
    pub progress: Option<ProgressState>,
    pub logs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct ProgressState {
    pub total_files: usize,
    pub processed: usize,
    pub skipped: usize,
    pub failed: usize,
}

impl AppState {
    pub fn push_log<S: Into<String>>(&mut self, msg: S) {
        self.logs.push(msg.into());
    }
}
