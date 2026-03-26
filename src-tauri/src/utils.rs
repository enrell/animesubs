use crate::models::*;
use encoding_rs::Encoding;
use chardetng::EncodingDetector;
use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub fn find_executable_in_path(names: &[&str]) -> Option<PathBuf> {
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

pub fn get_ffmpeg_path(custom_path: Option<String>) -> String {
    if let Some(path) = custom_path {
        if !path.is_empty() {
            return path;
        }
    }

    let exe_name = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(p) = find_executable_in_path(&[exe_name]) {
        return p.to_string_lossy().to_string();
    }

    if cfg!(windows) {
        let candidates = [
            r"C:\Program Files\FFmpeg\bin\ffmpeg.exe",
            r"C:\Program Files (x86)\FFmpeg\bin\ffmpeg.exe",
            r"C:\ffmpeg\bin\ffmpeg.exe",
            r"C:\tools\ffmpeg\bin\ffmpeg.exe",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return c.to_string();
            }
        }
    } else if cfg!(target_os = "macos") {
        let candidates = [
            "/opt/homebrew/bin/ffmpeg",
            "/usr/local/bin/ffmpeg",
            "/opt/local/bin/ffmpeg",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return c.to_string();
            }
        }
    }

    "ffmpeg".to_string()
}

pub fn get_ffprobe_path(custom_ffmpeg_path: Option<String>) -> String {
    if let Some(path) = custom_ffmpeg_path {
        if !path.is_empty() {
            let path = Path::new(&path);
            if let Some(parent) = path.parent() {
                let ffprobe_name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
                let ffprobe = parent.join(ffprobe_name);
                if ffprobe.exists() {
                    return ffprobe.to_string_lossy().to_string();
                }
            }
        }
    }

    let exe_name = if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" };
    if let Some(p) = find_executable_in_path(&[exe_name]) {
        return p.to_string_lossy().to_string();
    }

    if cfg!(windows) {
        let candidates = [
            r"C:\Program Files\FFmpeg\bin\ffprobe.exe",
            r"C:\Program Files (x86)\FFmpeg\bin\ffprobe.exe",
            r"C:\ffmpeg\bin\ffprobe.exe",
            r"C:\tools\ffmpeg\bin\ffprobe.exe",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return c.to_string();
            }
        }
    } else if cfg!(target_os = "macos") {
        let candidates = [
            "/opt/homebrew/bin/ffprobe",
            "/usr/local/bin/ffprobe",
            "/opt/local/bin/ffprobe",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return c.to_string();
            }
        }
    }

    "ffprobe".to_string()
}

pub fn resolve_mkvmerge_path() -> Option<String> {
    let exe_names: &[&str] = if cfg!(windows) {
        &["mkvmerge.exe"]
    } else {
        &["mkvmerge"]
    };

    if let Some(p) = find_executable_in_path(exe_names) {
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
    } else if cfg!(target_os = "macos") {
        let candidates = [
            "/opt/homebrew/bin/mkvmerge",
            "/usr/local/bin/mkvmerge",
            "/opt/local/bin/mkvmerge",
            "/Applications/MKVToolNix.app/Contents/MacOS/mkvmerge",
        ];
        for c in candidates {
            if Path::new(c).exists() {
                return Some(c.to_string());
            }
        }
    }

    None
}

pub fn strip_utf8_bom(mut content: String) -> String {
    if content.starts_with('\u{FEFF}') {
        content.remove(0);
    }
    content
}

pub fn read_file_as_utf8(file_path: &str) -> Result<String, String> {
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

pub fn write_utf8_file(path: &str, content: &str, include_bom: bool) -> Result<(), String> {
    let mut data = Vec::with_capacity(content.len() + if include_bom { 3 } else { 0 });
    if include_bom {
        data.extend_from_slice(b"\xEF\xBB\xBF");
    }
    data.extend_from_slice(content.as_bytes());
    fs::write(path, data).map_err(|e| format!("Failed to write subtitle file: {}", e))
}

pub fn convert_subtitle_to_utf8(subtitle_path: &str) -> Result<(String, Option<PathBuf>), String> {
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

    write_utf8_file(&temp_path.to_string_lossy(), &content, false)?;

    Ok((temp_path.to_string_lossy().to_string(), Some(temp_path)))
}

pub fn strip_ass_tags(text: &str) -> String {
    let tag_regex = Regex::new(r"\{[^}]*\}").unwrap();
    let result = tag_regex.replace_all(text, "");
    result.replace("\\N", "\n").replace("\\n", "\n")
}

pub fn is_music_or_karaoke_line(original_text: &str, clean_text: &str) -> bool {
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
        || (has_alignment_tag && is_very_short && looks_like_romaji)
        || ((is_very_short || mostly_short) && looks_like_romaji && repeating_tokens)
}

pub fn build_translation_prompt(style: &str, source_lang: &str, target_lang: &str) -> String {
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

pub fn clean_json_response(content: &str) -> String {
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

pub fn get_backup_dir(video_path: &str) -> PathBuf {
    let video_path = Path::new(video_path);
    let parent = video_path.parent().unwrap_or(Path::new("."));
    parent.join(".animesubs_backup")
}