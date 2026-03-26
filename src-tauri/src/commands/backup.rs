use crate::models::*;
use crate::utils::*;
use chrono;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[tauri::command]
pub async fn backup_subtitle(
    video_path: String,
    track_index: u32,
    ffmpeg_path: Option<String>,
) -> Result<BackupInfo, String> {
    let backup_dir = get_backup_dir(&video_path);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    let video_info = super::video::get_video_info(video_path.clone(), ffmpeg_path.clone()).await?;
    let track = video_info
        .subtitle_tracks
        .get(track_index as usize)
        .ok_or("Subtitle track not found")?;

    let format = match track.codec.as_str() {
        "ass" | "ssa" => "ass",
        "webvtt" => "vtt",
        _ => "srt",
    };

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let stem = Path::new(&video_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "video".to_string());
    let lang = track.language.as_deref().unwrap_or("und");

    let backup_filename = format!(
        "{}_{}_{}_track{}.{}",
        stem, lang, timestamp, track_index, format
    );
    let backup_path = backup_dir.join(&backup_filename);

    let result = super::subtitle::extract_subtitle(
        video_path.clone(),
        track_index,
        Some(backup_path.to_string_lossy().to_string()),
        Some(format.to_string()),
        ffmpeg_path,
    )
    .await?;

    if result.success {
        let backup_info = BackupInfo {
            original_path: video_path,
            backup_path: backup_path.to_string_lossy().to_string(),
            track_index,
            format: format.to_string(),
            created_at: timestamp,
        };

        let meta_path = backup_dir.join("backups.json");
        let mut backups: Vec<BackupInfo> = if meta_path.exists() {
            let content = fs::read_to_string(&meta_path).unwrap_or_else(|_| "[]".to_string());
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };

        backups.push(backup_info.clone());
        fs::write(&meta_path, serde_json::to_string_pretty(&backups).unwrap())
            .map_err(|e| format!("Failed to save backup metadata: {}", e))?;

        Ok(backup_info)
    } else {
        Err(result.error.unwrap_or_else(|| "Unknown error".to_string()))
    }
}

#[tauri::command]
pub async fn list_backups(video_path: String) -> Result<Vec<BackupInfo>, String> {
    let backup_dir = get_backup_dir(&video_path);
    let meta_path = backup_dir.join("backups.json");

    if !meta_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&meta_path)
        .map_err(|e| format!("Failed to read backup metadata: {}", e))?;

    let all_backups: Vec<BackupInfo> = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse backup metadata: {}", e))?;

    let video_path_normalized = Path::new(&video_path)
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or(video_path.clone());

    let backups: Vec<BackupInfo> = all_backups
        .into_iter()
        .filter(|b| {
            Path::new(&b.original_path)
                .canonicalize()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or(b.original_path.clone())
                == video_path_normalized
        })
        .collect();

    Ok(backups)
}

#[tauri::command]
pub async fn restore_subtitle(
    video_path: String,
    backup_path: String,
    _track_index: u32,
    ffmpeg_path: Option<String>,
) -> Result<OperationResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path);

    if !Path::new(&backup_path).exists() {
        return Err("Backup file not found".to_string());
    }

    let video_pathbuf = Path::new(&video_path);
    let parent = video_pathbuf.parent().unwrap_or(Path::new("."));
    let stem = video_pathbuf
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "video".to_string());
    let ext = video_pathbuf
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "mkv".to_string());

    let temp_output = parent.join(format!("{}_restored.{}", stem, ext));

    let result = Command::new(&ffmpeg)
        .args([
            "-i",
            &video_path,
            "-i",
            &backup_path,
            "-map",
            "0:v",
            "-map",
            "0:a",
            "-map",
            "1:0",
            "-c:v",
            "copy",
            "-c:a",
            "copy",
            "-c:s",
            "copy",
            "-y",
            temp_output.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if result.status.success() {
        fs::rename(&temp_output, &video_path)
            .map_err(|e| format!("Failed to replace original file: {}", e))?;

        Ok(OperationResult {
            success: true,
            message: "Subtitle restored successfully".to_string(),
            data: None,
        })
    } else {
        let _ = fs::remove_file(&temp_output);

        Ok(OperationResult {
            success: false,
            message: String::from_utf8_lossy(&result.stderr).to_string(),
            data: None,
        })
    }
}

#[tauri::command]
pub async fn delete_backup(backup_path: String, video_path: String) -> Result<OperationResult, String> {
    if Path::new(&backup_path).exists() {
        fs::remove_file(&backup_path)
            .map_err(|e| format!("Failed to delete backup file: {}", e))?;
    }

    let backup_dir = get_backup_dir(&video_path);
    let meta_path = backup_dir.join("backups.json");

    if meta_path.exists() {
        let content = fs::read_to_string(&meta_path).unwrap_or_else(|_| "[]".to_string());
        let mut backups: Vec<BackupInfo> = serde_json::from_str(&content).unwrap_or_default();

        backups.retain(|b| b.backup_path != backup_path);

        fs::write(&meta_path, serde_json::to_string_pretty(&backups).unwrap())
            .map_err(|e| format!("Failed to update backup metadata: {}", e))?;
    }

    Ok(OperationResult {
        success: true,
        message: "Backup deleted successfully".to_string(),
        data: None,
    })
}