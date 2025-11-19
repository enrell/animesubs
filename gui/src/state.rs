//! Pure application state & domain data.
//! Keep this file free of egui specifics when possible.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppState {
    pub project_label: String,
    pub selected_file: Option<PathBuf>,
    pub selected_folder: Option<PathBuf>,
    pub source_language: String,
    pub target_language: String,
    pub provider: String,
    pub api_key: String,
    pub model: String,
    pub base_url: String,
    pub preserve_honorifics: bool,
    pub dry_run: bool,
    pub is_processing: bool,
    pub progress: Option<ProgressState>,
    pub logs: Vec<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            project_label: String::new(),
            selected_file: None,
            selected_folder: None,
            source_language: "Japanese".to_string(),
            target_language: "pt-BR".to_string(),
            provider: "gemini".to_string(),
            api_key: String::new(),
            model: String::new(),
            base_url: String::new(),
            preserve_honorifics: true,
            dry_run: false,
            is_processing: false,
            progress: None,
            logs: Vec::new(),
        }
    }
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
