use chardetng::EncodingDetector;
use encoding_rs::Encoding;
use regex::Regex;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn create_command(program: impl AsRef<OsStr>) -> Command {
    #[cfg(not(windows))]
    {
        Command::new(program)
    }

    #[cfg(windows)]
    {
        let mut command = Command::new(program);
        command.creation_flags(CREATE_NO_WINDOW);
        command
    }
}

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

    let exe_name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
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

pub fn build_temp_subtitle_path(
    source_path: &str,
    label: &str,
    extension: &str,
) -> Result<PathBuf, String> {
    let temp_dir = env::temp_dir().join("animesubs");
    fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("Failed to create temporary directory: {}", e))?;

    let stem = Path::new(source_path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "subtitle".to_string());

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S_%3f");
    Ok(temp_dir.join(format!("{}_{}_{}.{}", stem, label, timestamp, extension)))
}

pub fn is_mkv_container(extension: &str) -> bool {
    matches!(extension.to_ascii_lowercase().as_str(), "mkv")
}

pub fn resolve_ffmpeg_subtitle_codec(
    container_ext: &str,
    subtitle_ext: &str,
) -> Result<&'static str, String> {
    match container_ext.to_ascii_lowercase().as_str() {
        "mkv" => Ok(match subtitle_ext {
            "ass" | "ssa" => "ass",
            "srt" | "subrip" => "srt",
            "vtt" | "webvtt" => "webvtt",
            _ => "srt",
        }),
        "mp4" | "m4v" | "mov" => Ok("mov_text"),
        "webm" => Ok("webvtt"),
        _ => Err(format!(
            "Embedding subtitles into .{} with ffmpeg is not supported \
             reliably. Use MKV/mkvmerge or disable embed for this file.",
            container_ext
        )),
    }
}

pub fn get_ffprobe_path(custom_ffmpeg_path: Option<String>) -> String {
    if let Some(path) = custom_ffmpeg_path {
        if !path.is_empty() {
            let path = Path::new(&path);
            if let Some(parent) = path.parent() {
                let ffprobe_name = if cfg!(windows) {
                    "ffprobe.exe"
                } else {
                    "ffprobe"
                };
                let ffprobe = parent.join(ffprobe_name);
                if ffprobe.exists() {
                    return ffprobe.to_string_lossy().to_string();
                }
            }
        }
    }

    let exe_name = if cfg!(windows) {
        "ffprobe.exe"
    } else {
        "ffprobe"
    };
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

pub fn build_translation_prompt(_style: &str, source_lang: &str, target_lang: &str) -> String {
    format!(
        r#"You are a professional subtitle translator. Translate from {} to {}.

Guidelines:
- Be faithful to the original meaning while producing natural, fluent \
translations that read like native dialogue in the target language.
- Adapt idioms, jokes, wordplay, and cultural references to their closest \
natural equivalent so the audience experiences the same intent and emotion.
- Preserve the tone, register, and emotional weight of each line — a formal \
character stays formal, a comedic line stays comedic, a tender moment \
stays tender.
- Keep character names in their original form unless a well-known localized \
version exists. Preserve Japanese honorifics (-san, -kun, -chan, -sama, \
-sensei, -senpai) and culturally specific terms that lack direct equivalents.
- Maintain subtitle readability: keep lines concise, preserve line breaks \
(\n), and ensure the text can be read in the time it appears on screen.
- If context metadata is provided (characters, terminology, synopsis), use \
it to keep names, terms, and tone consistent across episodes.
- Do NOT add explanations, notes, or commentary — only the translated text.

CRITICAL RULES:
1. You will receive a JSON array of subtitle lines with "id" and "text" fields
2. Return ONLY a valid JSON object with "translations" array containing \
objects with "id" and "text"
3. NEVER change line IDs - they must match exactly for correct subtitle \
replacement
4. If a line is clearly music/lyrics (karaoke tags, music notes, or \
opening/ending song cues), leave it unchanged
5. If a line contains only sound effects like "(笑)" or "♪", translate \
the sound description appropriately

Example input:
{{"lines": [{{"id": 0, "text": "Hello, how are you?"}}, {{"id": 1, "text": "I'm fine, thanks!"}}]}}

Example output:
{{"translations": [{{"id": 0, "text": "Translated line 0"}}, \
{{"id": 1, "text": "Translated line 1"}}]}}"#,
        source_lang, target_lang
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

pub fn format_context_profile(profile: &crate::models::ContextProfile) -> String {
    let mut sections: Vec<String> = Vec::new();

    if let Some(ref title) = profile.title {
        if !title.trim().is_empty() {
            sections.push(format!("Title: {}", title));
        }
    }

    if !profile.characters.is_empty() {
        let chars: Vec<String> = profile
            .characters
            .iter()
            .map(|c| {
                let verify_tag = if c.verified { " [verified]" } else { "" };
                let mut parts = vec![format!(
                    "  - {} ({}){}",
                    c.name,
                    c.role.as_deref().unwrap_or("unknown"),
                    verify_tag
                )];
                if let Some(ref desc) = c.description {
                    if !desc.trim().is_empty() {
                        parts.push(format!("    Description: {}", desc));
                    }
                }
                if let Some(ref rels) = c.relationships {
                    if !rels.is_empty() {
                        parts.push(format!("    Relationships: {}", rels.join(", ")));
                    }
                }
                parts.join("\n")
            })
            .collect();
        sections.push(format!("Characters:\n{}", chars.join("\n")));
    }

    if !profile.locations.is_empty() {
        sections.push(format!("Locations: {}", profile.locations.join(", ")));
    }

    if let Some(ref summary) = profile.plot_summary {
        if !summary.trim().is_empty() {
            sections.push(format!("Plot summary: {}", summary));
        }
    }

    if !profile.terminology.is_empty() {
        let terms: Vec<String> = profile
            .terminology
            .iter()
            .map(|t| {
                let base = format!("  - {} → {}", t.source_term, t.translated_term);
                if let Some(ref notes) = t.notes {
                    if !notes.trim().is_empty() {
                        format!("{} ({})", base, notes)
                    } else {
                        base
                    }
                } else {
                    base
                }
            })
            .collect();
        sections.push(format!("Terminology (use these translations consistently):\n{}", terms.join("\n")));
    }

    if let Some(ref tone) = profile.tone_notes {
        if !tone.trim().is_empty() {
            sections.push(format!("Tone: {}", tone));
        }
    }

    if let Some(ref custom) = profile.custom_instructions {
        if !custom.trim().is_empty() {
            sections.push(format!("Custom instructions: {}", custom));
        }
    }

    if let Some(ref media) = profile.media_match {
        let mut media_parts = vec![
            format!("  Title: {}", media.title_display),
            format!("  Provider: {} (ID: {})", media.provider, media.source_id),
        ];
        if let Some(ref t) = media.title_native {
            media_parts.push(format!("  Native title: {}", t));
        }
        if let Some(ref t) = media.title_english {
            media_parts.push(format!("  English title: {}", t));
        }
        if let Some(ref t) = media.title_romaji {
            media_parts.push(format!("  Romaji title: {}", t));
        }
        if !media.genres.is_empty() {
            media_parts.push(format!("  Genres: {}", media.genres.join(", ")));
        }
        if !media.tags.is_empty() {
            media_parts.push(format!("  Tags: {}", media.tags.join(", ")));
        }
        if !media.aliases.is_empty() {
            media_parts.push(format!("  Aliases: {}", media.aliases.join(", ")));
        }
        if let Some(ref synopsis) = media.synopsis {
            if !synopsis.trim().is_empty() {
                let preview: String = synopsis.chars().take(500).collect();
                media_parts.push(format!("  Synopsis: {}", preview));
            }
        }
        sections.push(format!(
            "Verified media data:\n{}",
            media_parts.join("\n")
        ));
    }

    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{CharacterProfile, ContextProfile, MediaMatch, TerminologyEntry};

    fn sample_profile() -> ContextProfile {
        ContextProfile {
            title: Some("Test Anime".to_string()),
            characters: vec![CharacterProfile {
                name: "Alice".to_string(),
                role: Some("protagonist".to_string()),
                description: Some("A brave girl".to_string()),
                relationships: Some(vec!["Friend of Bob".to_string()]),
                verified: true,
            }],
            locations: vec!["Tokyo".to_string(), "School".to_string()],
            plot_summary: Some("A girl saves the world".to_string()),
            terminology: vec![TerminologyEntry {
                source_term: "Mahou".to_string(),
                translated_term: "Magic".to_string(),
                notes: Some("Used in spell names".to_string()),
            }],
            tone_notes: Some("Casual and energetic".to_string()),
            custom_instructions: Some("Keep honorifics".to_string()),
            source_path: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            media_match: Some(MediaMatch {
                provider: "AniList".to_string(),
                source_id: "12345".to_string(),
                title_display: "Test Anime".to_string(),
                title_native: Some("テストアニメ".to_string()),
                title_english: Some("Test Anime".to_string()),
                title_romaji: Some("Tesuto Anime".to_string()),
                synopsis: Some("A test synopsis.".to_string()),
                genres: vec!["Action".to_string(), "Fantasy".to_string()],
                tags: vec![],
                aliases: vec![],
                format: Some("TV".to_string()),
                episodes: Some(12),
                season: Some("spring".to_string()),
                season_year: Some(2025),
            }),
        }
    }

    #[test]
    fn format_context_profile_includes_all_sections() {
        let formatted = format_context_profile(&sample_profile());

        assert!(formatted.contains("Title: Test Anime"));
        assert!(formatted.contains("Alice (protagonist) [verified]"));
        assert!(formatted.contains("A brave girl"));
        assert!(formatted.contains("Friend of Bob"));
        assert!(formatted.contains("Locations: Tokyo, School"));
        assert!(formatted.contains("Plot summary: A girl saves the world"));
        assert!(formatted.contains("Mahou → Magic"));
        assert!(formatted.contains("Used in spell names"));
        assert!(formatted.contains("Tone: Casual and energetic"));
        assert!(formatted.contains("Custom instructions: Keep honorifics"));
        assert!(formatted.contains("Verified media data:"));
        assert!(formatted.contains("AniList (ID: 12345)"));
        assert!(formatted.contains("Genres: Action, Fantasy"));
    }

    #[test]
    fn format_context_profile_skips_empty_sections() {
        let profile = ContextProfile {
            title: None,
            characters: vec![],
            locations: vec![],
            plot_summary: None,
            terminology: vec![],
            tone_notes: None,
            custom_instructions: None,
            source_path: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            media_match: None,
        };

        let formatted = format_context_profile(&profile);
        assert!(formatted.is_empty());
    }
}
