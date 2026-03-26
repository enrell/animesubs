use crate::models::*;
use crate::utils::*;
use regex::Regex;
use std::fs;
use std::path::Path;
use std::process::Command;
use tauri::command;

#[tauri::command]
pub async fn extract_subtitle(
    video_path: String,
    track_index: u32,
    output_path: Option<String>,
    format: Option<String>,
    ffmpeg_path: Option<String>,
) -> Result<ExtractResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path.clone());

    let video_info = super::video::get_video_info(video_path.clone(), ffmpeg_path).await?;

    let track = video_info
        .subtitle_tracks
        .get(track_index as usize)
        .ok_or("Subtitle track not found")?;

    let fmt = format.unwrap_or_else(|| "srt".to_string());

    let output = if let Some(out) = output_path {
        Path::new(&out).to_path_buf()
    } else {
        let video_pathbuf = Path::new(&video_path);
        let stem = video_pathbuf
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "subtitle".to_string());
        let lang = track.language.as_deref().unwrap_or("und");
        let parent = video_pathbuf.parent().unwrap_or(Path::new("."));
        parent.join(format!("{}.{}.{}", stem, lang, fmt))
    };

    let result = Command::new(&ffmpeg)
        .args([
            "-i",
            &video_path,
            "-map",
            &format!("0:s:{}", track_index),
            "-c:s",
            if fmt == "srt" {
                "srt"
            } else if fmt == "ass" {
                "ass"
            } else {
                "webvtt"
            },
            "-y",
            output.to_str().unwrap(),
        ])
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    if result.status.success() {
        Ok(ExtractResult {
            success: true,
            output_path: Some(output.to_string_lossy().to_string()),
            error: None,
        })
    } else {
        Ok(ExtractResult {
            success: false,
            output_path: None,
            error: Some(String::from_utf8_lossy(&result.stderr).to_string()),
        })
    }
}

fn parse_ass_file(content: &str) -> Result<SubtitleData, String> {
    let mut lines: Vec<DialogLine> = Vec::new();
    let mut in_events = false;
    let mut header_end = 0;

    let skip_styles: Vec<&str> = vec![
        "op", "ed", "opening", "ending", "karaoke", "romaji", "japanese", "sign", "signs", "title",
        "song", "lyrics", "insert", "credit", "credits",
    ];

    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();

        if trimmed.starts_with("[Events]") {
            in_events = true;
            header_end = line_num;
            continue;
        }

        if in_events && trimmed.starts_with("[") {
            break;
        }

        if in_events && trimmed.starts_with("Dialogue:") {
            let parts: Vec<&str> = trimmed.splitn(10, ',').collect();
            if parts.len() >= 10 {
                let start = parts[1].trim().to_string();
                let end = parts[2].trim().to_string();
                let style = Some(parts[3].trim().to_string());
                let name = {
                    let n = parts[4].trim();
                    if n.is_empty() {
                        None
                    } else {
                        Some(n.to_string())
                    }
                };
                let original_text = parts[9..].join(",");
                let clean_text = strip_ass_tags(&original_text);
                let is_music_line = is_music_or_karaoke_line(&original_text, &clean_text);

                let style_lower = style.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                let should_skip_style = skip_styles.iter().any(|&skip| {
                    style_lower.contains(skip)
                        || style_lower.split_whitespace().any(|word| word == skip)
                });

                let is_too_short = clean_text.trim().chars().count() < 3;

                if !clean_text.trim().is_empty()
                    && !should_skip_style
                    && !is_too_short
                    && !is_music_line
                {
                    lines.push(DialogLine {
                        index: lines.len(),
                        text: clean_text,
                        original_with_formatting: original_text,
                        start,
                        end,
                        style,
                        name,
                    });
                }
            }
        }
    }

    let header: String = content
        .lines()
        .take(header_end + 2)
        .collect::<Vec<&str>>()
        .join("\n");

    Ok(SubtitleData {
        format: "ass".to_string(),
        line_count: lines.len(),
        lines,
        source_path: String::new(),
        ass_header: Some(header),
    })
}

fn parse_srt_file(content: &str) -> Result<SubtitleData, String> {
    let mut lines: Vec<DialogLine> = Vec::new();
    let mut current_index: Option<usize> = None;
    let mut current_start = String::new();
    let mut current_end = String::new();
    let mut current_text = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if let Ok(idx) = trimmed.parse::<usize>() {
            if current_index.is_some() && !current_text.is_empty() {
                let text = current_text.join("\n");
                if !text.trim().is_empty() && !is_music_or_karaoke_line(&text, &text) {
                    lines.push(DialogLine {
                        index: lines.len(),
                        text: text.clone(),
                        original_with_formatting: text,
                        start: current_start.clone(),
                        end: current_end.clone(),
                        style: None,
                        name: None,
                    });
                }
            }
            current_index = Some(idx);
            current_text.clear();
            continue;
        }

        if trimmed.contains("-->") {
            let parts: Vec<&str> = trimmed.split("-->").collect();
            if parts.len() >= 2 {
                current_start = parts[0].trim().to_string();
                current_end = parts[1].trim().to_string();
            }
            continue;
        }

        if current_index.is_some() && !trimmed.is_empty() {
            let tag_regex = Regex::new(r"<[^>]*>").unwrap();
            let clean = tag_regex.replace_all(trimmed, "").to_string();
            current_text.push(clean);
        }
    }

    if current_index.is_some() && !current_text.is_empty() {
        let text = current_text.join("\n");
        if !text.trim().is_empty() && !is_music_or_karaoke_line(&text, &text) {
            lines.push(DialogLine {
                index: lines.len(),
                text: text.clone(),
                original_with_formatting: text,
                start: current_start,
                end: current_end,
                style: None,
                name: None,
            });
        }
    }

    Ok(SubtitleData {
        format: "srt".to_string(),
        line_count: lines.len(),
        lines,
        source_path: String::new(),
        ass_header: None,
    })
}

fn parse_vtt_file(content: &str) -> Result<SubtitleData, String> {
    let mut lines: Vec<DialogLine> = Vec::new();
    let mut current_start = String::new();
    let mut current_end = String::new();
    let mut current_text = Vec::new();
    let mut in_cue = false;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("WEBVTT") || trimmed.starts_with("NOTE") {
            continue;
        }

        if trimmed.contains("-->") {
            if in_cue && !current_text.is_empty() {
                let text = current_text.join("\n");
                if !text.trim().is_empty() && !is_music_or_karaoke_line(&text, &text) {
                    lines.push(DialogLine {
                        index: lines.len(),
                        text: text.clone(),
                        original_with_formatting: text,
                        start: current_start.clone(),
                        end: current_end.clone(),
                        style: None,
                        name: None,
                    });
                }
                current_text.clear();
            }

            let parts: Vec<&str> = trimmed.split("-->").collect();
            if parts.len() >= 2 {
                current_start = parts[0].trim().to_string();
                current_end = parts[1]
                    .trim()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_string();
            }
            in_cue = true;
            continue;
        }

        if trimmed.is_empty() && in_cue {
            if !current_text.is_empty() {
                let text = current_text.join("\n");
                if !text.trim().is_empty() && !is_music_or_karaoke_line(&text, &text) {
                    lines.push(DialogLine {
                        index: lines.len(),
                        text: text.clone(),
                        original_with_formatting: text,
                        start: current_start.clone(),
                        end: current_end.clone(),
                        style: None,
                        name: None,
                    });
                }
                current_text.clear();
            }
            in_cue = false;
            continue;
        }

        if in_cue && !trimmed.is_empty() {
            let tag_regex = Regex::new(r"<[^>]*>").unwrap();
            let clean = tag_regex.replace_all(trimmed, "").to_string();
            current_text.push(clean);
        }
    }

    if !current_text.is_empty() {
        let text = current_text.join("\n");
        if !text.trim().is_empty() && !is_music_or_karaoke_line(&text, &text) {
            lines.push(DialogLine {
                index: lines.len(),
                text: text.clone(),
                original_with_formatting: text,
                start: current_start,
                end: current_end,
                style: None,
                name: None,
            });
        }
    }

    Ok(SubtitleData {
        format: "vtt".to_string(),
        line_count: lines.len(),
        lines,
        source_path: String::new(),
        ass_header: None,
    })
}

#[tauri::command]
pub async fn parse_subtitle_file(file_path: String) -> Result<SubtitleData, String> {
    let content = read_file_as_utf8(&file_path)?;

    let ext = Path::new(&file_path)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let mut data = match ext.as_str() {
        "ass" | "ssa" => parse_ass_file(&content)?,
        "srt" => parse_srt_file(&content)?,
        "vtt" | "webvtt" => parse_vtt_file(&content)?,
        _ => return Err(format!("Unsupported subtitle format: {}", ext)),
    };

    data.source_path = file_path;
    Ok(data)
}