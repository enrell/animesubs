use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use futures::future::join_all;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

// ============================================================================
// Data Structures
// ============================================================================

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleTrack {
    pub index: u32,
    pub stream_index: u32,
    pub codec: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub default: bool,
    pub forced: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct VideoInfo {
    pub path: String,
    pub filename: String,
    pub duration: Option<f64>,
    pub subtitle_tracks: Vec<SubtitleTrack>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BackupInfo {
    pub original_path: String,
    pub backup_path: String,
    pub track_index: u32,
    pub format: String,
    pub created_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExtractResult {
    pub success: bool,
    pub output_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OperationResult {
    pub success: bool,
    pub message: String,
    pub data: Option<String>,
}

// ============================================================================
// Translation Pipeline Data Structures
// ============================================================================

/// A single dialog line extracted from subtitles
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DialogLine {
    /// Unique index for this line (for consistent batch translation)
    pub index: usize,
    /// The original text content (without formatting codes)
    pub text: String,
    /// Original text with formatting preserved (for ASS override tags)
    pub original_with_formatting: String,
    /// Start time
    pub start: String,
    /// End time  
    pub end: String,
    /// Style name (for ASS)
    pub style: Option<String>,
    /// Speaker/actor name
    pub name: Option<String>,
}

/// Extracted subtitle data with metadata
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SubtitleData {
    /// Format: "ass", "srt", "vtt"
    pub format: String,
    /// All dialog lines
    pub lines: Vec<DialogLine>,
    /// Total line count
    pub line_count: usize,
    /// Original file path
    pub source_path: String,
    /// For ASS: preserve script info and styles
    pub ass_header: Option<String>,
}

/// Translation request for LLM
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationRequest {
    /// Batch of lines to translate
    pub lines: Vec<TranslationLine>,
    /// Source language
    pub source_lang: String,
    /// Target language
    pub target_lang: String,
    /// Translation style/system prompt name
    pub style: String,
}

/// Single line in translation batch
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationLine {
    /// Line index for matching
    pub id: usize,
    /// Text to translate
    pub text: String,
}

/// LLM translation response (structured output)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationResponse {
    pub translations: Vec<TranslatedLine>,
}

/// Single translated line
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslatedLine {
    pub id: usize,
    pub text: String,
}

/// Translation progress event
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TranslationProgress {
    pub current_batch: usize,
    pub total_batches: usize,
    pub lines_translated: usize,
    pub total_lines: usize,
    pub status: String,
}

/// LLM Provider configuration
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LLMConfig {
    pub provider: String,
    pub api_key: String,
    pub endpoint: String,
    pub model: String,
    pub system_prompt: String,
}

// ============================================================================
// FFmpeg Path Resolution
// ============================================================================

fn get_ffmpeg_path(custom_path: Option<String>) -> String {
    if let Some(path) = custom_path {
        if !path.is_empty() {
            return path;
        }
    }
    if cfg!(windows) {
        let candidates = [
            r"C:\Program Files\FFmpeg\bin\ffmpeg.exe",
            r"C:\Program Files (x86)\FFmpeg\bin\ffmpeg.exe",
            r"C:\ffmpeg\bin\ffmpeg.exe",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return c.to_string();
            }
        }
    }
    "ffmpeg".to_string()
}

fn get_ffprobe_path(custom_ffmpeg_path: Option<String>) -> String {
    if let Some(path) = custom_ffmpeg_path {
        if !path.is_empty() {
            // Try to derive ffprobe path from ffmpeg path
            let path = Path::new(&path);
            if let Some(parent) = path.parent() {
                let ffprobe = parent.join("ffprobe");
                if ffprobe.exists() {
                    return ffprobe.to_string_lossy().to_string();
                }
            }
        }
    }
    "ffprobe".to_string()
}

// ============================================================================
// FFprobe Commands
// ============================================================================

#[tauri::command]
async fn get_video_info(
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
async fn scan_folder_for_videos(folder_path: String) -> Result<Vec<String>, String> {
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

// ============================================================================
// Subtitle Extraction
// ============================================================================

#[tauri::command]
async fn extract_subtitle(
    video_path: String,
    track_index: u32,
    output_path: Option<String>,
    format: Option<String>,
    ffmpeg_path: Option<String>,
) -> Result<ExtractResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path.clone());

    // Get video info to find the actual stream index
    let video_info = get_video_info(video_path.clone(), ffmpeg_path).await?;

    let track = video_info
        .subtitle_tracks
        .get(track_index as usize)
        .ok_or("Subtitle track not found")?;

    let fmt = format.unwrap_or_else(|| "srt".to_string());

    let output = if let Some(out) = output_path {
        PathBuf::from(out)
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

// ============================================================================
// Subtitle Parsing - Extract dialog text only
// ============================================================================

/// Strip ASS override tags from text, keeping only the displayable content
fn strip_ass_tags(text: &str) -> String {
    let tag_regex = Regex::new(r"\{[^}]*\}").unwrap();
    let result = tag_regex.replace_all(text, "");
    // Also handle \N (ASS newline) -> actual newline for translation
    result.replace("\\N", "\n").replace("\\n", "\n")
}

/// Heuristic to detect music/lyrics lines (OP/ED songs, karaoke, BGM cues)
fn is_music_or_karaoke_line(original_text: &str, clean_text: &str) -> bool {
    let lowered = clean_text.to_ascii_lowercase();
    let original_lower = original_text.to_ascii_lowercase();

    let has_music_notes = clean_text.contains('♪')
        || clean_text.contains('♫')
        || clean_text.contains('♩')
        || clean_text.contains('♬');
    let has_music_words = lowered.contains("[music")
        || lowered.contains("(music")
        || lowered.contains("bgm")
        || lowered.contains("instrumental")
        || lowered.contains("ending theme")
        || lowered.contains("opening theme");
    let has_karaoke_tags = original_lower.contains("\\k");
    let has_alignment_tag = original_lower.contains("\\an");

    let trimmed = clean_text.trim();
    let is_very_short = trimmed.chars().count() <= 3;
    let looks_like_romaji = trimmed
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c.is_whitespace());

    // Catch karaoke syllables after format conversion (ASS -> SRT/VTT) where tags are lost
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    let short_tokens = tokens.iter().filter(|t| t.chars().count() <= 3).count();
    let mostly_short = tokens.len() >= 2 && short_tokens * 2 >= tokens.len();
    let repeating_tokens = if tokens.len() >= 3 {
        let unique = tokens.iter().collect::<std::collections::HashSet<_>>();
        unique.len() * 2 <= tokens.len()
    } else {
        false
    };

    has_music_notes
        || has_music_words
        || has_karaoke_tags
        // Very short romaji lines that are top-aligned are usually karaoke syllables
        || (has_alignment_tag && is_very_short && looks_like_romaji)
        // Repeated short syllables without alignment tags (post-conversion)
        || ((is_very_short || mostly_short) && looks_like_romaji && repeating_tokens)
}

/// Parse ASS file and extract dialog lines
fn parse_ass_file(content: &str) -> Result<SubtitleData, String> {
    let mut lines: Vec<DialogLine> = Vec::new();
    let mut in_events = false;
    let mut header_end = 0;

    // Styles to skip (OP/ED lyrics, karaoke, signs, etc.)
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
            // New section started, stop parsing events
            break;
        }

        if in_events && trimmed.starts_with("Dialogue:") {
            // Parse dialogue line
            // Format: Dialogue: Layer,Start,End,Style,Name,MarginL,MarginR,MarginV,Effect,Text
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

                // Skip lines with styles that indicate OP/ED/karaoke/signs
                let style_lower = style.as_ref().map(|s| s.to_lowercase()).unwrap_or_default();
                let should_skip_style = skip_styles.iter().any(|&skip| {
                    style_lower.contains(skip)
                        || style_lower.split_whitespace().any(|word| word == skip)
                });

                // Skip very short lines (likely karaoke syllables) - less than 3 chars after stripping
                let is_too_short = clean_text.trim().chars().count() < 3;

                // Skip empty or formatting-only lines
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

    // Extract header (everything before [Events])
    let header: String = content
        .lines()
        .take(header_end + 2) // Include [Events] and Format line
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

/// Parse SRT file and extract dialog lines
fn parse_srt_file(content: &str) -> Result<SubtitleData, String> {
    let mut lines: Vec<DialogLine> = Vec::new();
    let mut current_index: Option<usize> = None;
    let mut current_start = String::new();
    let mut current_end = String::new();
    let mut current_text = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();

        // Try to parse as index number
        if let Ok(idx) = trimmed.parse::<usize>() {
            // Save previous entry if exists
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

        // Try to parse as timestamp line (00:00:00,000 --> 00:00:00,000)
        if trimmed.contains("-->") {
            let parts: Vec<&str> = trimmed.split("-->").collect();
            if parts.len() >= 2 {
                current_start = parts[0].trim().to_string();
                current_end = parts[1].trim().to_string();
            }
            continue;
        }

        // Otherwise it's text content
        if current_index.is_some() && !trimmed.is_empty() {
            // Remove HTML-like tags from SRT
            let tag_regex = Regex::new(r"<[^>]*>").unwrap();
            let clean = tag_regex.replace_all(trimmed, "").to_string();
            current_text.push(clean);
        }
    }

    // Don't forget the last entry
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

/// Parse WebVTT file and extract dialog lines
fn parse_vtt_file(content: &str) -> Result<SubtitleData, String> {
    let mut lines: Vec<DialogLine> = Vec::new();
    let mut current_start = String::new();
    let mut current_end = String::new();
    let mut current_text = Vec::new();
    let mut in_cue = false;

    for line in content.lines() {
        let trimmed = line.trim();

        // Skip WEBVTT header and empty lines before first cue
        if trimmed.starts_with("WEBVTT") || trimmed.starts_with("NOTE") {
            continue;
        }

        // Check for timestamp line (00:00:00.000 --> 00:00:00.000)
        if trimmed.contains("-->") {
            // Save previous entry if exists
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
                // VTT may have position info after timestamp
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

        // Empty line ends current cue
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

        // Text content
        if in_cue && !trimmed.is_empty() {
            // Remove VTT tags
            let tag_regex = Regex::new(r"<[^>]*>").unwrap();
            let clean = tag_regex.replace_all(trimmed, "").to_string();
            current_text.push(clean);
        }
    }

    // Don't forget the last entry
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

/// Parse any subtitle file based on extension
fn strip_utf8_bom(mut content: String) -> String {
    if content.starts_with('\u{FEFF}') {
        content.remove(0);
    }
    content
}

fn read_file_as_utf8(file_path: &str) -> Result<String, String> {
    let bytes = fs::read(file_path).map_err(|e| format!("Failed to read subtitle file: {}", e))?;

    if let Some((encoding, _)) = Encoding::for_bom(&bytes) {
        let (decoded, _) = encoding.decode_with_bom_removal(&bytes);
        return Ok(decoded.into_owned());
    }

    if let Ok(content) = String::from_utf8(bytes.clone()) {
        return Ok(strip_utf8_bom(content));
    }

    let mut detector = EncodingDetector::new();
    detector.feed(&bytes, true);
    let encoding = detector.guess(None, true);
    let (decoded, _) = encoding.decode_with_bom_removal(&bytes);
    Ok(decoded.into_owned())
}

fn write_utf8_file(path: &str, content: &str, include_bom: bool) -> Result<(), String> {
    let mut data = Vec::with_capacity(content.len() + if include_bom { 3 } else { 0 });
    if include_bom {
        data.extend_from_slice(b"\xEF\xBB\xBF");
    }
    data.extend_from_slice(content.as_bytes());
    fs::write(path, data).map_err(|e| format!("Failed to write subtitle file: {}", e))
}

/// Convert a text-based subtitle to UTF-8 and return the path to the UTF-8 copy.
/// Non-text subtitle formats are returned as-is.
fn convert_subtitle_to_utf8(subtitle_path: &str) -> Result<(String, Option<PathBuf>), String> {
    let ext = Path::new(subtitle_path)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();

    let is_text_sub = matches!(ext.as_str(), "ass" | "ssa" | "srt" | "vtt" | "webvtt");
    if !is_text_sub {
        return Ok((subtitle_path.to_string(), None));
    }

    let content = read_file_as_utf8(subtitle_path)?;
    let path = Path::new(subtitle_path);
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "subtitle".to_string());
    let temp_path = parent.join(format!("{}_utf8.{}", stem, ext));

    // Matroska text subs are expected to be UTF-8 without BOM to avoid player quirks.
    write_utf8_file(&temp_path.to_string_lossy(), &content, false)?;

    Ok((temp_path.to_string_lossy().to_string(), Some(temp_path)))
}

fn find_executable_in_path(names: &[&str]) -> Option<PathBuf> {
    if let Some(paths) = env::var_os("PATH") {
        for p in env::split_paths(&paths) {
            for name in names {
                let candidate = p.join(name);
                if candidate.exists() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn resolve_mkvmerge_path() -> Option<String> {
    if let Some(p) = find_executable_in_path(&["mkvmerge", "mkvmerge.exe"]) {
        return Some(p.to_string_lossy().to_string());
    }
    if cfg!(windows) {
        let candidates = [
            r"C:\Program Files\MKVToolNix\mkvmerge.exe",
            r"C:\Program Files (x86)\MKVToolNix\mkvmerge.exe",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return Some(c.to_string());
            }
        }
    }
    None
}

#[tauri::command]
async fn parse_subtitle_file(file_path: String) -> Result<SubtitleData, String> {
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

// ============================================================================
// LLM Translation Pipeline
// ============================================================================

/// Build system prompt for translation
fn build_translation_prompt(style: &str, source_lang: &str, target_lang: &str) -> String {
    let base_instruction = format!(
        "You are a professional subtitle translator. Translate from {} to {}.",
        source_lang, target_lang
    );

    let style_instruction = match style {
        "natural" => "Translate naturally, prioritizing how native speakers actually talk. Adapt idioms, jokes, and cultural references to feel native in the target language while preserving the original meaning and tone.",
        "literal" => "Translate as literally as possible while still being grammatically correct. Preserve the original sentence structure and word choices where feasible.",
        "localized" => "Fully localize the content. Adapt cultural references, names, jokes, and idioms to equivalents that work in the target culture. The goal is for the translation to feel like it was originally written in the target language.",
        "formal" => "Use formal, polite language appropriate for professional or official contexts. Avoid slang, contractions, and casual expressions.",
        "casual" => "Use casual, conversational language. Feel free to use contractions, common expressions, and a friendly tone.",
        "honorifics" => "Preserve Japanese honorifics (san, kun, chan, sama, sensei, senpai) and cultural terms that don't have direct equivalents. Add brief context in parentheses if needed for clarity.",
        _ => "Translate naturally, balancing accuracy with readability.",
    };

    format!(
        r#"{}

Style: {}

CRITICAL RULES:
1. You will receive a JSON array of subtitle lines with "id" and "text" fields
2. Return ONLY a valid JSON object with "translations" array containing objects with "id" and "text"
3. NEVER change line IDs - they must match exactly for correct subtitle replacement
4. Keep translations concise - subtitles need to be readable quickly
5. Preserve line breaks (\n) where present in the source
6. Do not add explanations or notes - only the translated text
7. If a line contains only sound effects like "(笑)" or "♪", translate the sound description appropriately
8. If a line is clearly music/lyrics (karaoke tags, music notes, or ending/opening song cues), leave it unchanged

Example input:
{{"lines": [{{"id": 0, "text": "Hello, how are you?"}}, {{"id": 1, "text": "I'm fine, thanks!"}}]}}

Example output:
{{"translations": [{{"id": 0, "text": "Translated line 0"}}, {{"id": 1, "text": "Translated line 1"}}]}}"#,
        base_instruction, style_instruction
    )
}

/// Call LLM API for translation
async fn call_llm_api(
    config: &LLMConfig,
    lines: &[TranslationLine],
    source_lang: &str,
    target_lang: &str,
) -> Result<Vec<TranslatedLine>, String> {
    let client = reqwest::Client::new();

    let system_prompt = build_translation_prompt(&config.system_prompt, source_lang, target_lang);

    let user_content = serde_json::json!({
        "lines": lines
    });

    // Detect if Gemini is using OpenAI-compatible endpoint
    let is_gemini_openai_compat =
        config.provider == "gemini" && config.endpoint.contains("/openai");

    // Build request based on provider
    let request_body = match config.provider.as_str() {
        "openai" | "openrouter" | _ if is_gemini_openai_compat => {
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content.to_string()}
                ],
                "temperature": 0.3,
                "response_format": {"type": "json_object"}
            })
        }
        "gemini" => {
            serde_json::json!({
                "contents": [{
                    "parts": [{
                        "text": format!("{}\n\nTranslate the following:\n{}", system_prompt, user_content)
                    }]
                }],
                "generationConfig": {
                    "temperature": 0.3,
                    "responseMimeType": "application/json"
                }
            })
        }
        "ollama" | "lmstudio" => {
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content.to_string()}
                ],
                "stream": false,
                "format": "json",
                "options": {
                    "temperature": 0.3
                }
            })
        }
        _ => return Err(format!("Unsupported provider: {}", config.provider)),
    };

    // Build the endpoint URL
    let endpoint_url = if is_gemini_openai_compat {
        // OpenAI-compatible Gemini endpoint needs /chat/completions
        let base = config.endpoint.trim_end_matches('/');
        format!("{}/chat/completions", base)
    } else if config.provider == "gemini" {
        // Native Gemini endpoint uses API key as query param
        format!("{}:generateContent?key={}", config.endpoint, config.api_key)
    } else {
        // For OpenAI-compatible endpoints, append /chat/completions if needed
        let base = config.endpoint.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        }
    };

    let mut request = client.post(&endpoint_url).json(&request_body);

    // Add auth headers based on provider
    if is_gemini_openai_compat {
        // Gemini OpenAI-compat uses Bearer token
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match config.provider.as_str() {
            "openai" | "openrouter" => {
                request = request.header("Authorization", format!("Bearer {}", config.api_key));
                if config.provider == "openrouter" {
                    request = request.header("HTTP-Referer", "https://animesubs.app");
                }
            }
            "gemini" => {
                // Native Gemini uses API key in URL (handled above)
            }
            _ => {}
        }
    }

    eprintln!(
        "Calling LLM API: {} with model {}",
        endpoint_url, config.model
    );

    let response = request
        .send()
        .await
        .map_err(|e| format!("Failed to call LLM API: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("LLM API error ({}): {}", status, error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse LLM response: {}", e))?;

    // Extract content based on provider format
    let content = if is_gemini_openai_compat
        || config.provider == "openai"
        || config.provider == "openrouter"
    {
        response_json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or("Missing content in OpenAI response")?
            .to_string()
    } else if config.provider == "gemini" {
        response_json["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or("Missing content in Gemini response")?
            .to_string()
    } else if config.provider == "ollama" || config.provider == "lmstudio" {
        response_json["message"]["content"]
            .as_str()
            .ok_or("Missing content in Ollama response")?
            .to_string()
    } else {
        return Err("Unsupported provider".to_string());
    };

    eprintln!("LLM response content: {}", content);

    let cleaned_content = clean_json_response(&content);

    let translation_response: TranslationResponse = serde_json::from_str(&cleaned_content)
        .map_err(|e| {
            format!(
                "Failed to parse translation JSON: {}. Response was: {}",
                e, cleaned_content
            )
        })?;

    Ok(translation_response.translations)
}

fn clean_json_response(content: &str) -> String {
    let content = content.trim();

    if let Some(start) = content.find('{') {
        let mut depth = 0;
        let mut end_pos = start;

        for (i, c) in content[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end_pos = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if end_pos > start {
            return content[start..end_pos].to_string();
        }
    }

    content.to_string()
}

#[tauri::command]
async fn translate_subtitles(
    app: AppHandle,
    subtitle_data: SubtitleData,
    config: LLMConfig,
    source_lang: String,
    target_lang: String,
    batch_size: Option<usize>,
    concurrency: Option<usize>,
    request_delay: Option<u64>,
) -> Result<SubtitleData, String> {
    let batch_size = batch_size.unwrap_or(20);
    let concurrency = concurrency.unwrap_or(1).max(1).min(10); // Clamp between 1-10
    let request_delay_ms = request_delay.unwrap_or(0);
    let total_lines = subtitle_data.lines.len();

    if total_lines == 0 {
        return Err("No dialog lines to translate".to_string());
    }

    let mut translated_lines = subtitle_data.lines.clone();
    let translation_map: Arc<Mutex<HashMap<usize, String>>> = Arc::new(Mutex::new(HashMap::new()));
    let completed_batches: Arc<Mutex<usize>> = Arc::new(Mutex::new(0));

    let batches: Vec<(usize, Vec<TranslationLine>)> = subtitle_data
        .lines
        .chunks(batch_size)
        .enumerate()
        .map(|(idx, chunk)| {
            let batch_lines: Vec<TranslationLine> = chunk
                .iter()
                .map(|line| TranslationLine {
                    id: line.index,
                    text: line.text.clone(),
                })
                .collect();
            (idx, batch_lines)
        })
        .collect();

    let total_batches = batches.len();

    for batch_group in batches.chunks(concurrency) {
        let mut handles = Vec::new();

        for (batch_idx, batch_lines) in batch_group {
            let config = config.clone();
            let source_lang = source_lang.clone();
            let target_lang = target_lang.clone();
            let translation_map = Arc::clone(&translation_map);
            let completed_batches = Arc::clone(&completed_batches);
            let app = app.clone();
            let batch_idx = *batch_idx;
            let batch_lines = batch_lines.clone();

            let handle = tokio::spawn(async move {
                match call_llm_api(&config, &batch_lines, &source_lang, &target_lang).await {
                    Ok(translations) => {
                        let mut map = translation_map.lock().await;
                        for translated in translations {
                            map.insert(translated.id, translated.text);
                        }

                        let mut completed = completed_batches.lock().await;
                        *completed += 1;

                        // Emit progress event
                        let progress = TranslationProgress {
                            current_batch: *completed,
                            total_batches,
                            lines_translated: map.len(),
                            total_lines,
                            status: "translating".to_string(),
                        };

                        let _ = app.emit("translation-progress", &progress);
                        eprintln!("Translation progress: {:?}", progress);

                        Ok(())
                    }
                    Err(e) => {
                        let _ = app.emit(
                            "translation-error",
                            format!("Batch {} failed: {}", batch_idx + 1, e),
                        );
                        Err(format!(
                            "Translation failed at batch {}: {}",
                            batch_idx + 1,
                            e
                        ))
                    }
                }
            });

            handles.push(handle);
        }

        let results = join_all(handles).await;

        for result in results {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(e)) => return Err(e),
                Err(e) => return Err(format!("Task panicked: {}", e)),
            }
        }

        if request_delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(request_delay_ms)).await;
        }
    }

    // Apply translations
    let map = translation_map.lock().await;
    for line in &mut translated_lines {
        if let Some(translated_text) = map.get(&line.index) {
            line.text = translated_text.clone();
        }
    }

    Ok(SubtitleData {
        format: subtitle_data.format,
        line_count: translated_lines.len(),
        lines: translated_lines,
        source_path: subtitle_data.source_path,
        ass_header: subtitle_data.ass_header,
    })
}

// ============================================================================
// Subtitle Reconstruction - Replace dialog with translations
// ============================================================================

/// Reconstruct ASS file with translated dialog
fn reconstruct_ass(original_content: &str, translations: &[DialogLine]) -> String {
    let mut result = Vec::new();
    let mut in_events = false;
    let mut in_styles = false;
    let mut style_encoding_index: Option<usize> = None;

    // Build a map from original text -> translated text for matching
    let translation_map: std::collections::HashMap<String, &str> = translations
        .iter()
        .map(|t| {
            let key = strip_ass_tags(&t.original_with_formatting)
                .trim()
                .to_lowercase();
            (key, t.text.as_str())
        })
        .collect();

    // Styles to skip (OP/ED lyrics, karaoke, signs, etc.) - same as in parsing
    let skip_styles: Vec<&str> = vec![
        "op", "ed", "opening", "ending", "karaoke", "romaji", "japanese", "sign", "signs", "title",
        "song", "lyrics", "insert", "credit", "credits",
    ];

    for line in original_content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("[") {
            let section = trimmed.trim_matches(&['[', ']'][..]).to_ascii_lowercase();
            match section.as_str() {
                "v4+ styles" => {
                    in_styles = true;
                    in_events = false;
                    style_encoding_index = None;
                }
                "events" => {
                    in_styles = false;
                    in_events = true;
                }
                _ => {
                    in_styles = false;
                    in_events = false;
                }
            }
            result.push(line.to_string());
            continue;
        }

        if in_styles {
            let lower = trimmed.to_ascii_lowercase();
            if lower.starts_with("format:") {
                let fields: Vec<String> = trimmed[7..]
                    .split(',')
                    .map(|f| f.trim().to_string())
                    .collect();
                style_encoding_index = fields
                    .iter()
                    .position(|f| f.eq_ignore_ascii_case("Encoding"));
                result.push(line.to_string());
                continue;
            } else if lower.starts_with("style:") {
                if let Some(idx) = style_encoding_index {
                    let mut values: Vec<String> = trimmed[6..]
                        .split(',')
                        .map(|v| v.trim().to_string())
                        .collect();
                    if idx < values.len() {
                        values[idx] = "0".to_string();
                    }
                    let new_line = format!("Style: {}", values.join(","));
                    result.push(new_line);
                    continue;
                }
            }
        }

        if in_events && trimmed.starts_with("Dialogue:") {
            // Parse and reconstruct the dialogue line
            let parts: Vec<&str> = trimmed.splitn(10, ',').collect();
            if parts.len() >= 10 {
                let original_text = parts[9..].join(",");
                let clean_original = strip_ass_tags(&original_text);
                let style = parts[3].trim().to_lowercase();
                let is_music_line = is_music_or_karaoke_line(&original_text, &clean_original);

                // Check if this style should be skipped
                let should_skip = skip_styles.iter().any(|&skip| {
                    style.contains(skip) || style.split_whitespace().any(|word| word == skip)
                });

                let is_too_short = clean_original.trim().chars().count() < 3;

                if !should_skip
                    && !is_too_short
                    && !clean_original.trim().is_empty()
                    && !is_music_line
                {
                    let lookup_key = clean_original.trim().to_lowercase();
                    if let Some(translated_text) = translation_map.get(&lookup_key) {
                        let new_text = apply_ass_formatting(&original_text, translated_text);
                        let new_line = format!("{},{}", parts[..9].join(","), new_text);
                        result.push(new_line);
                        continue;
                    }
                }
            }
        }

        result.push(line.to_string());
    }

    result.join("\n")
}

fn apply_ass_formatting(original: &str, translated: &str) -> String {
    let tag_regex = Regex::new(r"^(\{[^}]*\})").unwrap();
    let leading_tags: String = tag_regex.find_iter(original).map(|m| m.as_str()).collect();

    // Convert newlines back to \N for ASS
    let formatted_translation = translated.replace("\n", "\\N");

    if !leading_tags.is_empty() {
        format!("{}{}", leading_tags, formatted_translation)
    } else {
        formatted_translation
    }
}

fn reconstruct_srt(translations: &[DialogLine]) -> String {
    let mut result = Vec::new();

    for (idx, line) in translations.iter().enumerate() {
        result.push(format!("{}", idx + 1));
        result.push(format!("{} --> {}", line.start, line.end));
        result.push(line.text.clone());
        result.push(String::new()); // Empty line between entries
    }

    result.join("\n")
}

fn reconstruct_vtt(translations: &[DialogLine]) -> String {
    let mut result = vec!["WEBVTT".to_string(), String::new()];

    for line in translations {
        result.push(format!("{} --> {}", line.start, line.end));
        result.push(line.text.clone());
        result.push(String::new());
    }

    result.join("\n")
}

#[tauri::command]
async fn save_translated_subtitles(
    translated_data: SubtitleData,
    output_path: String,
    original_file_path: Option<String>,
) -> Result<OperationResult, String> {
    let content = match translated_data.format.as_str() {
        "ass" | "ssa" => {
            if let Some(original_path) = original_file_path {
                let original_content = read_file_as_utf8(&original_path)?;
                reconstruct_ass(&original_content, &translated_data.lines)
            } else if let Some(header) = &translated_data.ass_header {
                let mut result = header.clone();
                result.push_str("\n");
                for line in &translated_data.lines {
                    result.push_str(&format!(
                        "Dialogue: 0,{},{},{},{},0,0,0,,{}\n",
                        line.start,
                        line.end,
                        line.style.as_deref().unwrap_or("Default"),
                        line.name.as_deref().unwrap_or(""),
                        line.text.replace("\n", "\\N")
                    ));
                }
                result
            } else {
                return Err("Cannot reconstruct ASS without original file or header".to_string());
            }
        }
        "srt" => reconstruct_srt(&translated_data.lines),
        "vtt" | "webvtt" => reconstruct_vtt(&translated_data.lines),
        _ => return Err(format!("Unsupported format: {}", translated_data.format)),
    };

    write_utf8_file(&output_path, &content, true)?;

    Ok(OperationResult {
        success: true,
        message: format!("Saved translated subtitles to {}", output_path),
        data: Some(output_path),
    })
}

// ============================================================================
// Backup & Restore
// ============================================================================

fn get_backup_dir(video_path: &str) -> PathBuf {
    let video_path = Path::new(video_path);
    let parent = video_path.parent().unwrap_or(Path::new("."));
    parent.join(".animesubs_backup")
}

#[tauri::command]
async fn backup_subtitle(
    video_path: String,
    track_index: u32,
    ffmpeg_path: Option<String>,
) -> Result<BackupInfo, String> {
    let backup_dir = get_backup_dir(&video_path);
    fs::create_dir_all(&backup_dir)
        .map_err(|e| format!("Failed to create backup directory: {}", e))?;

    let video_info = get_video_info(video_path.clone(), ffmpeg_path.clone()).await?;
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

    let result = extract_subtitle(
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
async fn list_backups(video_path: String) -> Result<Vec<BackupInfo>, String> {
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
async fn restore_subtitle(
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
        // Replace original with restored version
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
async fn delete_backup(backup_path: String, video_path: String) -> Result<OperationResult, String> {
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

// ============================================================================
// Subtitle Embedding
// ============================================================================

#[tauri::command]
async fn embed_subtitle(
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

    let video_info = get_video_info(video_path.clone(), Some(ffmpeg.clone())).await?;
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
async fn remove_subtitle_track(
    video_path: String,
    track_index: u32,
    ffmpeg_path: Option<String>,
) -> Result<OperationResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path.clone());

    let video_info = get_video_info(video_path.clone(), ffmpeg_path).await?;

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

// ============================================================================
// Utility Commands
// ============================================================================

#[tauri::command]
async fn check_ffmpeg(ffmpeg_path: Option<String>) -> Result<OperationResult, String> {
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
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

// ============================================================================
// App Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            get_video_info,
            scan_folder_for_videos,
            extract_subtitle,
            backup_subtitle,
            list_backups,
            restore_subtitle,
            delete_backup,
            embed_subtitle,
            remove_subtitle_track,
            check_ffmpeg,
            // Translation pipeline
            parse_subtitle_file,
            translate_subtitles,
            save_translated_subtitles,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
