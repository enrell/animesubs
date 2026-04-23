use crate::models::*;
use crate::utils::*;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[tauri::command]
pub async fn embed_subtitle(
    video_path: String,
    subtitle_path: String,
    language: Option<String>,
    title: Option<String>,
    set_default: bool,
    ffmpeg_path: Option<String>,
    use_mkvmerge: Option<bool>,
) -> Result<OperationResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path);
    let mut use_mkvmerge = use_mkvmerge.unwrap_or(true);
    let mkvmerge_path = resolve_mkvmerge_path();

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

    let temp_output = parent.join(format!("{}_with_subs.{}", stem, ext));

    let (utf8_subtitle_path, temp_utf8_path) = convert_subtitle_to_utf8(&subtitle_path)?;

    if use_mkvmerge && mkvmerge_path.is_none() {
        eprintln!("mkvmerge not available, falling back to ffmpeg for embedding");
        use_mkvmerge = false;
    }

    if use_mkvmerge {
        let lang_opt = language.unwrap_or_else(|| "und".to_string());
        let title_val = title.unwrap_or_else(|| "Translated".to_string());
        let default_flag = if set_default { "0:1" } else { "0:0" };

        let args = vec![
            "-o".to_string(),
            temp_output.to_string_lossy().to_string(),
            video_path.clone(),
            "--language".to_string(),
            format!("0:{}", lang_opt),
            "--track-name".to_string(),
            format!("0:{}", title_val),
            "--default-track".to_string(),
            default_flag.to_string(),
            utf8_subtitle_path.clone(),
        ];

        let mkvmerge_bin = mkvmerge_path.unwrap_or_else(|| "mkvmerge".to_string());

        let result = Command::new(&mkvmerge_bin)
            .args(&args)
            .output()
            .map_err(|e| format!("Failed to run mkvmerge: {}", e))?;

        if let Some(temp_path) = &temp_utf8_path {
            let _ = fs::remove_file(temp_path);
        }

        if result.status.success() {
            fs::rename(&temp_output, &video_path)
                .map_err(|e| format!("Failed to replace original file: {}", e))?;

            return Ok(OperationResult {
                success: true,
                message: "Subtitle embedded successfully (mkvmerge)".to_string(),
                data: None,
            });
        } else {
            let _ = fs::remove_file(&temp_output);
            return Ok(OperationResult {
                success: false,
                message: String::from_utf8_lossy(&result.stderr).to_string(),
                data: None,
            });
        }
    }

    let sub_ext = Path::new(&utf8_subtitle_path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let sub_codec = match sub_ext.as_str() {
        "ass" | "ssa" => "ass",
        "srt" | "subrip" => "srt",
        "vtt" | "webvtt" => "webvtt",
        _ => "copy",
    };

    let mut args = vec![
        "-i".to_string(),
        video_path.clone(),
        "-i".to_string(),
        utf8_subtitle_path.clone(),
        "-map".to_string(),
        "0".to_string(),
        "-map".to_string(),
        "1:0".to_string(),
        "-c".to_string(),
        "copy".to_string(),
    ];

    let video_info = super::video::get_video_info(video_path.clone(), Some(ffmpeg.clone())).await?;
    let new_track_idx = video_info.subtitle_tracks.len();

    args.push(format!("-c:s:{}", new_track_idx));
    args.push(sub_codec.to_string());

    if let Some(lang) = language {
        args.push(format!("-metadata:s:s:{}", new_track_idx));
        args.push(format!("language={}", lang));
    }

    let title_val = title.unwrap_or_else(|| "Translated".to_string());
    args.push(format!("-metadata:s:s:{}", new_track_idx));
    args.push(format!("title={}", title_val));

    if set_default {
        args.push(format!("-disposition:s:{}", new_track_idx));
        args.push("default".to_string());
    }

    args.push("-y".to_string());
    args.push(temp_output.to_string_lossy().to_string());

    let result = Command::new(&ffmpeg)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if let Some(temp_path) = &temp_utf8_path {
        let _ = fs::remove_file(temp_path);
    }

    if result.status.success() {
        fs::rename(&temp_output, &video_path)
            .map_err(|e| format!("Failed to replace original file: {}", e))?;

        Ok(OperationResult {
            success: true,
            message: "Subtitle embedded successfully".to_string(),
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
pub async fn remove_subtitle_track(
    video_path: String,
    track_index: u32,
    ffmpeg_path: Option<String>,
) -> Result<OperationResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path.clone());

    let video_info = super::video::get_video_info(video_path.clone(), ffmpeg_path).await?;

    if track_index as usize >= video_info.subtitle_tracks.len() {
        return Err("Invalid track index".to_string());
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

    let temp_output = parent.join(format!("{}_modified.{}", stem, ext));

    let mut args = vec![
        "-i".to_string(),
        video_path.clone(),
        "-map".to_string(),
        "0:v".to_string(),
        "-map".to_string(),
        "0:a".to_string(),
    ];

    for (i, _) in video_info.subtitle_tracks.iter().enumerate() {
        if i != track_index as usize {
            args.push("-map".to_string());
            args.push(format!("0:s:{}", i));
        }
    }

    args.extend([
        "-c".to_string(),
        "copy".to_string(),
        "-y".to_string(),
        temp_output.to_string_lossy().to_string(),
    ]);

    let result = Command::new(&ffmpeg)
        .args(&args)
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if result.status.success() {
        fs::rename(&temp_output, &video_path)
            .map_err(|e| format!("Failed to replace original file: {}", e))?;

        Ok(OperationResult {
            success: true,
            message: "Subtitle track removed successfully".to_string(),
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