use crate::models::*;
use crate::utils::*;
use std::process::Command;
use tauri::command;

#[tauri::command]
pub async fn check_ffmpeg(ffmpeg_path: Option<String>) -> Result<OperationResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path);

    let result = Command::new(&ffmpeg).arg("-version").output();

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