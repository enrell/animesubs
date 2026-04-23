use crate::models::*;
use crate::utils::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[tauri::command]
pub async fn get_video_info(
    video_path: String,
    ffmpeg_path: Option<String>,
) -> Result<VideoInfo, String> {
    let ffprobe = get_ffprobe_path(ffmpeg_path);

    let output = Command::new(&ffprobe)
        .args([
            "-v",
            "quiet",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            &video_path,
        ])
        .output()
        .map_err(|e| format!("Failed to run ffprobe: {}. Is FFmpeg installed?", e))?;

    if !output.status.success() {
        return Err(format!(
            "ffprobe failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json_str = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .map_err(|e| format!("Failed to parse ffprobe output: {}", e))?;

    let streams = json["streams"]
        .as_array()
        .ok_or("No streams found in video")?;

    let mut subtitle_tracks: Vec<SubtitleTrack> = Vec::new();
    let mut sub_index = 0u32;

    for stream in streams {
        if stream["codec_type"].as_str() == Some("subtitle") {
            let tags = &stream["tags"];
            subtitle_tracks.push(SubtitleTrack {
                index: sub_index,
                stream_index: stream["index"].as_u64().unwrap_or(0) as u32,
                codec: stream["codec_name"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string(),
                language: tags["language"].as_str().map(String::from),
                title: tags["title"].as_str().map(String::from),
                default: stream["disposition"]["default"].as_i64() == Some(1),
                forced: stream["disposition"]["forced"].as_i64() == Some(1),
            });
            sub_index += 1;
        }
    }

    let duration = json["format"]["duration"]
        .as_str()
        .and_then(|d| d.parse::<f64>().ok());

    let filename = Path::new(&video_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| video_path.clone());

    Ok(VideoInfo {
        path: video_path,
        filename,
        duration,
        subtitle_tracks,
    })
}

#[tauri::command]
pub async fn scan_folder_for_videos(folder_path: String) -> Result<Vec<String>, String> {
    let video_extensions = ["mkv", "mp4", "webm", "avi", "mov", "wmv", "flv", "m4v"];
    let mut videos: Vec<String> = Vec::new();

    let entries =
        fs::read_dir(&folder_path).map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext_str = ext.to_string_lossy().to_lowercase();
                if video_extensions.contains(&ext_str.as_str()) {
                    videos.push(path.to_string_lossy().to_string());
                }
            }
        }
    }

    videos.sort();
    Ok(videos)
}