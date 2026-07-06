use crate::models::*;
use std::fs;
use std::path::Path;
use tauri::AppHandle;
use tauri::Emitter;

#[tauri::command]
pub async fn extract_context_profile(
    app: AppHandle,
    request: ContextExtractionRequest,
) -> Result<ContextExtractionResult, String> {
    let _ = app.emit("context-extraction-progress", "Analyzing subtitles...");

    let mut profile = crate::providers::extract_context(&request).await?;

    if let Some(custom) = request.custom_instructions.as_deref() {
        profile.custom_instructions = Some(custom.to_string());
    }
    profile.source_path = Some(request.subtitle_data.source_path.clone());

    let _ = app.emit("context-extraction-progress", "done");

    Ok(ContextExtractionResult { profile })
}

#[tauri::command]
pub async fn save_context_profile(
    profile: ContextProfile,
    output_path: String,
) -> Result<OperationResult, String> {
    let json = serde_json::to_string_pretty(&profile)
        .map_err(|e| format!("Failed to serialize context profile: {}", e))?;

    let path = Path::new(&output_path);
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }
    }

    fs::write(path, &json)
        .map_err(|e| format!("Failed to write context profile: {}", e))?;

    Ok(OperationResult {
        success: true,
        message: format!("Context profile saved to {}", output_path),
        data: Some(output_path),
    })
}

#[tauri::command]
pub async fn load_context_profile(file_path: String) -> Result<ContextProfile, String> {
    let content = fs::read_to_string(&file_path)
        .map_err(|e| format!("Failed to read context profile: {}", e))?;

    let profile: ContextProfile = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse context profile: {}", e))?;

    Ok(profile)
}
