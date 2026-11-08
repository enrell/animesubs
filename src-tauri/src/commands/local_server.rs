//! The local animesubs server (see `docs/ARCHITECTURE.md`).
//!
//! The app connects to a server that is already running for this user, or
//! starts the bundled `animesubs-cli serve` and stops it again on exit. The
//! existing translation flow does not depend on it yet; when the server
//! binary is missing the app keeps working and reports it as unavailable.

use animesubs_client::{ensure_server, Connection, EnsureOptions};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Mutex;

#[derive(Default)]
pub struct LocalServerState {
    connection: Mutex<Option<Connection>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalServerStatus {
    pub available: bool,
    /// Whether the app started this server (and will stop it on exit).
    pub owned: bool,
    pub url: Option<String>,
    pub version: Option<String>,
    pub message: String,
}

fn home_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    // Same directory the server and CLI use by default; honours ANIMESUBS_HOME.
    match animesubs_api::paths::default_home() {
        Some(home) => Ok(home),
        None => app
            .path()
            .app_config_dir()
            .map_err(|e| format!("Failed to resolve app config directory: {}", e)),
    }
}

/// Connect to (or start) the local server and report its status.
#[tauri::command]
pub async fn local_server_status(
    app: AppHandle,
    state: State<'_, LocalServerState>,
) -> Result<LocalServerStatus, String> {
    let mut slot = state.connection.lock().await;

    if let Some(connection) = slot.as_ref() {
        if let Ok(health) = connection.client.health().await {
            return Ok(connected(connection, health.version));
        }
    }
    // Gone (crashed or stopped from the CLI): forget it and try again.
    if let Some(stale) = slot.take() {
        stale.close().await;
    }

    let options = EnsureOptions::new(home_dir(&app)?);
    match ensure_server(options).await {
        Ok(connection) => {
            let version = connection
                .client
                .health()
                .await
                .map(|health| health.version)
                .ok();
            let status = connected(&connection, version.unwrap_or_default());
            *slot = Some(connection);
            Ok(status)
        }
        Err(error) => Ok(LocalServerStatus {
            available: false,
            owned: false,
            url: None,
            version: None,
            message: error.to_string(),
        }),
    }
}

fn connected(connection: &Connection, version: String) -> LocalServerStatus {
    let owned = connection.is_owned();
    LocalServerStatus {
        available: true,
        owned,
        url: Some(connection.client.base_url().to_string()),
        version: Some(version),
        message: if owned {
            "Started local server".to_string()
        } else {
            "Connected to running local server".to_string()
        },
    }
}

/// Stop the server if the app started it. Called once, on app exit.
pub fn stop_owned_server(app: &AppHandle) {
    let state = app.state::<LocalServerState>();
    tauri::async_runtime::block_on(async {
        if let Some(connection) = state.connection.lock().await.take() {
            connection.close().await;
        }
    });
}
