use crate::models::*;
use crate::utils::*;
use reqwest::Client;
use std::fs;
use std::path::Path;
use tauri::{AppHandle, Manager};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn secrets_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("Failed to resolve app config directory: {}", e))?;
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create app config directory: {}", e))?;
    Ok(config_dir.join("secrets.json"))
}

fn read_secrets(app: &AppHandle) -> Result<serde_json::Map<String, serde_json::Value>, String> {
    let path = secrets_path(app)?;
    if !path.exists() {
        return Ok(serde_json::Map::new());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read secrets file: {}", e))?;
    serde_json::from_str(&content).map_err(|e| format!("Failed to parse secrets file: {}", e))
}

fn write_secrets(
    app: &AppHandle,
    secrets: &serde_json::Map<String, serde_json::Value>,
) -> Result<(), String> {
    let path = secrets_path(app)?;
    let data = serde_json::to_string_pretty(secrets)
        .map_err(|e| format!("Failed to serialize secrets: {}", e))?;
    fs::write(&path, data).map_err(|e| format!("Failed to write secrets file: {}", e))?;

    #[cfg(unix)]
    {
        let mut permissions = fs::metadata(&path)
            .map_err(|e| format!("Failed to read secrets permissions: {}", e))?
            .permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&path, permissions)
            .map_err(|e| format!("Failed to restrict secrets permissions: {}", e))?;
    }

    Ok(())
}

#[tauri::command]
pub async fn check_ffmpeg(ffmpeg_path: Option<String>) -> Result<OperationResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path);

    let result = create_command(&ffmpeg).arg("-version").output();

    match result {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout);
            let first_line = version.lines().next().unwrap_or("FFmpeg found");

            Ok(OperationResult {
                success: true,
                message: first_line.to_string(),
                data: Some(ffmpeg),
            })
        }
        _ => Ok(OperationResult {
            success: false,
            message: "FFmpeg not found. Please install FFmpeg or specify its path.".to_string(),
            data: None,
        }),
    }
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub async fn delete_file(file_path: String) -> Result<OperationResult, String> {
    let path = Path::new(&file_path);

    if !path.exists() {
        return Ok(OperationResult {
            success: true,
            message: "File already removed".to_string(),
            data: Some(file_path),
        });
    }

    fs::remove_file(path).map_err(|e| format!("Failed to delete file: {}", e))?;

    Ok(OperationResult {
        success: true,
        message: "File deleted successfully".to_string(),
        data: Some(file_path),
    })
}

#[tauri::command]
pub async fn load_api_key(app: AppHandle, provider: String) -> Result<OperationResult, String> {
    let secrets = read_secrets(&app)?;
    let api_key = secrets
        .get(&provider)
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();

    Ok(OperationResult {
        success: true,
        message: "API key loaded".to_string(),
        data: Some(api_key),
    })
}

#[tauri::command]
pub async fn save_api_key(
    app: AppHandle,
    provider: String,
    api_key: String,
) -> Result<OperationResult, String> {
    let mut secrets = read_secrets(&app)?;
    if api_key.is_empty() {
        secrets.remove(&provider);
    } else {
        secrets.insert(provider, serde_json::Value::String(api_key));
    }
    write_secrets(&app, &secrets)?;

    Ok(OperationResult {
        success: true,
        message: "API key saved".to_string(),
        data: None,
    })
}

#[derive(serde::Serialize, Clone, Debug)]
pub struct ModelEntry {
    pub label: String,
    pub value: String,
}

#[tauri::command]
pub async fn fetch_models(
    endpoint: String,
    api_key: Option<String>,
    provider: Option<String>,
) -> Result<Vec<ModelEntry>, String> {
    let client = Client::new();
    let provider = provider
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    // Gemini uses the native API for listing models
    let (url, use_bearer) = if provider == "gemini" {
        let key = api_key.as_deref().unwrap_or_default();
        (
            format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                key
            ),
            false,
        )
    } else {
        let base = endpoint.trim_end_matches('/');
        (format!("{}/models", base), true)
    };

    let mut request = client.get(&url);

    if use_bearer {
        if let Some(ref key) = api_key {
            if !key.is_empty() {
                request = request.header("Authorization", format!("Bearer {}", key));
            }
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to fetch models: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Models API error ({}): {}", status, error_text));
    }

    let data: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse models response: {}", e))?;

    let mut models: Vec<ModelEntry> = Vec::new();

    // OpenAI / OpenAI-compatible format: { "data": [{ "id": "..." }] }
    if let Some(arr) = data.get("data").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(id) = item.get("id").and_then(|v| v.as_str()) {
                models.push(ModelEntry {
                    label: id.to_string(),
                    value: id.to_string(),
                });
            }
        }
    }
    // Ollama format: { "models": [{ "name": "..." }] }
    else if let Some(arr) = data.get("models").and_then(|v| v.as_array()) {
        for item in arr {
            let name = item
                .get("name")
                .or_else(|| item.get("model"))
                .and_then(|v| v.as_str());
            if let Some(name) = name {
                models.push(ModelEntry {
                    label: name.to_string(),
                    value: name.to_string(),
                });
            }
        }
    }
    // Gemini format: array of { "name": "models/gemini-..." }
    else if let Some(arr) = data.as_array() {
        for item in arr {
            if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                if name.contains("gemini") {
                    let clean = name.strip_prefix("models/").unwrap_or(name);
                    models.push(ModelEntry {
                        label: clean.to_string(),
                        value: clean.to_string(),
                    });
                }
            }
        }
    }

    models.sort_by(|a, b| a.label.cmp(&b.label));
    Ok(models)
}
