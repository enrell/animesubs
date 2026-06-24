use crate::models::*;
use crate::utils::*;
use regex::Regex;
use std::path::Path;
use std::sync::LazyLock;

#[tauri::command]
pub async fn extract_subtitle(
    video_path: String,
    track_index: u32,
    output_path: Option<String>,
    format: Option<String>,
    temporary: Option<bool>,
    ffmpeg_path: Option<String>,
) -> Result<ExtractResult, String> {
    let ffmpeg = get_ffmpeg_path(ffmpeg_path.clone());

    let video_info = super::video::get_video_info(video_path.clone(), ffmpeg_path).await?;

    let track = video_info
        .subtitle_tracks
        .get(track_index as usize)
        .ok_or("Subtitle track not found")?;

    let fmt = resolve_extraction_format(format.as_deref(), &track.codec);

    let output = if let Some(out) = output_path {
        Path::new(&out).to_path_buf()
    } else if temporary.unwrap_or(false) {
        build_temp_subtitle_path(&video_path, &format!("extract_track{}", track_index), &fmt)?
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

    let result = create_command(&ffmpeg)
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

fn resolve_extraction_format(format: Option<&str>, codec: &str) -> String {
    match format.map(|value| value.trim().to_ascii_lowercase()) {
        Some(value) if !value.is_empty() && value != "auto" => value,
        _ => {
            let codec = codec.to_ascii_lowercase();
            if codec.contains("ass") || codec.contains("ssa") {
                "ass".to_string()
            } else if codec.contains("webvtt") || codec.contains("vtt") {
                "vtt".to_string()
            } else {
                "srt".to_string()
            }
        }
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

static HTML_TAG_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]*>").unwrap());

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
            let clean = HTML_TAG_RE.replace_all(trimmed, "").to_string();
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
                current_end = parts[1].split_whitespace().next().unwrap_or("").to_string();
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
            let clean = HTML_TAG_RE.replace_all(trimmed, "").to_string();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_srt_strips_tags_and_skips_music_lines() {
        let content = r#"1
00:00:01,000 --> 00:00:02,000
<i>Hello there</i>

2
00:00:03,000 --> 00:00:04,000
♪ la la la ♪

3
00:00:05,000 --> 00:00:06,500
General Kenobi
"#;

        let data = parse_srt_file(content).unwrap();

        assert_eq!(data.format, "srt");
        assert_eq!(data.line_count, 2);
        assert_eq!(data.lines[0].index, 0);
        assert_eq!(data.lines[0].text, "Hello there");
        assert_eq!(data.lines[0].start, "00:00:01,000");
        assert_eq!(data.lines[0].end, "00:00:02,000");
        assert_eq!(data.lines[1].text, "General Kenobi");
    }

    #[test]
    fn parse_vtt_reads_cues_and_strips_inline_tags() {
        let content = r#"WEBVTT

00:00:01.000 --> 00:00:02.000 align:start
<c.yellow>Hello</c>

00:00:03.000 --> 00:00:04.000
World
"#;

        let data = parse_vtt_file(content).unwrap();

        assert_eq!(data.format, "vtt");
        assert_eq!(data.line_count, 2);
        assert_eq!(data.lines[0].text, "Hello");
        assert_eq!(data.lines[0].end, "00:00:02.000");
        assert_eq!(data.lines[1].text, "World");
    }

    #[test]
    fn parse_ass_preserves_dialogue_metadata_and_skips_sign_styles() {
        let content = r#"[Script Info]
Title: Example

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, Encoding
Style: Default,Arial,20,&H00FFFFFF,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:02.00,Default,Alice,0,0,0,,{\i1}Hello\Nthere
Dialogue: 0,0:00:03.00,0:00:04.00,Signs,,0,0,0,,Shop sign
Dialogue: 0,0:00:05.00,0:00:06.00,Default,,0,0,0,,♪ la la ♪
"#;

        let data = parse_ass_file(content).unwrap();

        assert_eq!(data.format, "ass");
        assert_eq!(data.line_count, 1);
        assert_eq!(data.lines[0].text, "Hello\nthere");
        assert_eq!(
            data.lines[0].original_with_formatting,
            "{\\i1}Hello\\Nthere"
        );
        assert_eq!(data.lines[0].style.as_deref(), Some("Default"));
        assert_eq!(data.lines[0].name.as_deref(), Some("Alice"));
        assert!(data.ass_header.unwrap().contains("[Events]"));
    }

    #[test]
    fn parse_ass_reads_dialogue_with_complex_override_tags() {
        let first_dialogue = concat!(
            "Dialogue: 0,0:00:01.00,0:00:04.00,Default,,0,0,0,,",
            "{\\t(25,2235,\\fscx109.48\\fscy109.48)",
            "\\blur0.3\\fs27\\c&H434343&\\frz-5.26\\fax0.05",
            "\\move(883.5,453,877.26,449.01,25,2235)}",
            "Served By: Yamada\n",
        );
        let second_dialogue = concat!(
            "Dialogue: 0,0:00:05.00,0:00:08.00,Default,,0,0,0,,",
            "{\\fad(500,0)\\c&H4A6EE1&}\"",
            "{\\c&H07E1C8&}M{\\c&H4A6EE1&}o",
            "{\\c&H07E1C8&}v{\\c&H4A6EE1&}i",
            "{\\c&H07E1C8&}n{\\c&H4A6EE1&}g ",
            "{\\c&H07E1C8&}a{\\c&H4A6EE1&}n",
            "{\\c&H07E1C8&}d {\\c&H4A6EE1&}G",
            "{\\c&H07E1C8&}i{\\c&H4A6EE1&}r",
            "{\\c&H07E1C8&}l{\\c&H4A6EE1&}f",
            "{\\c&H07E1C8&}r{\\c&H4A6EE1&}i",
            "{\\c&H07E1C8&}e{\\c&H4A6EE1&}n",
            "{\\c&H07E1C8&}d{\\c&H4A6EE1&}\"\n",
        );
        let content = format!(
            r#"[Script Info]
Title: Issue 5

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
"#,
        ) + first_dialogue
            + second_dialogue;

        let data = parse_ass_file(&content).unwrap();

        assert_eq!(data.line_count, 2);
        assert_eq!(data.lines[0].text, "Served By: Yamada");
        assert_eq!(data.lines[1].text, "\"Moving and Girlfriend\"");
    }

    #[test]
    fn auto_extraction_format_keeps_ass_tracks_as_ass() {
        assert_eq!(resolve_extraction_format(None, "ass"), "ass");
        assert_eq!(resolve_extraction_format(Some(""), "ssa"), "ass");
        assert_eq!(resolve_extraction_format(Some(" Auto "), "webvtt"), "vtt");
        assert_eq!(resolve_extraction_format(Some("srt"), "ass"), "srt");
    }
}
