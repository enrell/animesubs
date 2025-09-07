//! Actions: mutating operations separated from UI composition.
//! These would eventually call into the Python translation backend (via FFI/CLI/API).

use crate::AppState;
use anyhow::{Result, bail};
use std::path::Path;

pub fn select_folder(state: &mut AppState, folder: impl AsRef<Path>) -> Result<()> {
    let path = folder.as_ref().to_path_buf();
    state.selected_folder = Some(path.clone());
    state.push_log(format!("Selected folder: {}", path.display()));
    Ok(())
}

pub fn start_processing(state: &mut AppState) -> Result<()> {
    if state.selected_folder.is_none() {
        bail!("Select a folder first");
    }
    state.is_processing = true;
    state.progress = Some(crate::state::ProgressState {
        total_files: 0,
        processed: 0,
        skipped: 0,
        failed: 0,
    });
    state.push_log("Started processing (placeholder)");
    // TODO: spawn thread / async task
    Ok(())
}
