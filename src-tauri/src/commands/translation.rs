use crate::models::*;
use crate::providers::call_llm_api;
use crate::utils::*;
use futures::future::join_all;
use regex::Regex;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn translate_subtitles(
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
    let concurrency = concurrency.unwrap_or(1).clamp(1, 10);
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

    let map = translation_map.lock().await;
    let mut changed_lines = 0usize;
    for line in &mut translated_lines {
        if let Some(translated_text) = map.get(&line.index) {
            if translated_text.trim() != line.text.trim() {
                changed_lines += 1;
            }
            line.text = translated_text.clone();
        }
    }

    if changed_lines == 0 {
        return Err(
            "Translation produced no subtitle changes. Check the provider, model, prompt, and selected languages."
                .to_string(),
        );
    }

    Ok(SubtitleData {
        format: subtitle_data.format,
        line_count: translated_lines.len(),
        lines: translated_lines,
        source_path: subtitle_data.source_path,
        ass_header: subtitle_data.ass_header,
    })
}

fn reconstruct_ass(original_content: &str, translations: &[DialogLine]) -> String {
    let mut result = Vec::new();
    let mut in_events = false;
    let mut in_styles = false;
    let mut style_encoding_index: Option<usize> = None;

    let translation_map: std::collections::HashMap<String, &str> = translations
        .iter()
        .map(|t| {
            let key = strip_ass_tags(&t.original_with_formatting)
                .trim()
                .to_lowercase();
            (key, t.text.as_str())
        })
        .collect();

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
            let parts: Vec<&str> = trimmed.splitn(10, ',').collect();
            if parts.len() >= 10 {
                let original_text = parts[9..].join(",");
                let clean_original = strip_ass_tags(&original_text);
                let style = parts[3].trim().to_lowercase();
                let is_music_line = is_music_or_karaoke_line(&original_text, &clean_original);

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
        result.push(String::new());
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
pub async fn save_translated_subtitles(
    translated_data: SubtitleData,
    output_path: Option<String>,
    original_file_path: Option<String>,
    temporary: Option<bool>,
) -> Result<OperationResult, String> {
    let has_translated_changes = translated_data
        .lines
        .iter()
        .any(|line| line.text.trim() != strip_ass_tags(&line.original_with_formatting).trim());

    if !has_translated_changes {
        return Err(
            "Refusing to save translated subtitles because no translated lines differ from the source."
                .to_string(),
        );
    }

    let content = match translated_data.format.as_str() {
        "ass" | "ssa" => {
            if let Some(ref original_path) = original_file_path {
                let original_content = read_file_as_utf8(original_path)?;
                reconstruct_ass(&original_content, &translated_data.lines)
            } else if let Some(header) = &translated_data.ass_header {
                let mut result = header.clone();
                result.push('\n');
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

    let resolved_output_path = if let Some(path) = output_path {
        path
    } else if temporary.unwrap_or(false) {
        let source_path = original_file_path
            .as_deref()
            .filter(|p| !p.is_empty())
            .unwrap_or(&translated_data.source_path);
        build_temp_subtitle_path(source_path, "translated", &translated_data.format)?
            .to_string_lossy()
            .to_string()
    } else {
        return Err("output_path is required when temporary save is disabled".to_string());
    };

    write_utf8_file(&resolved_output_path, &content, true)?;

    Ok(OperationResult {
        success: true,
        message: format!("Saved translated subtitles to {}", resolved_output_path),
        data: Some(resolved_output_path),
    })
}

fn emit_job_progress(
    app: &AppHandle,
    current_file: usize,
    total_files: usize,
    progress: f64,
    status: impl Into<String>,
) {
    let _ = app.emit(
        "translation-job-progress",
        TranslationJobProgress {
            current_file,
            total_files,
            progress: progress.clamp(0.0, 100.0),
            status: status.into(),
        },
    );
}

fn normalize_language_key(value: &str) -> String {
    value.to_lowercase().replace('_', "-").trim().to_string()
}

fn to_ffmpeg_lang_code(value: Option<&str>) -> String {
    let Some(value) = value else {
        return "und".to_string();
    };
    if value.is_empty() {
        return "und".to_string();
    }

    let normalized = normalize_language_key(value);
    let mapped = match normalized.as_str() {
        "und" => Some("und"),
        "en" | "eng" | "en-us" => Some("eng"),
        "ja" | "jpn" => Some("jpn"),
        "zh" | "zh-cn" | "zh-tw" => Some("zho"),
        "ko" | "kor" => Some("kor"),
        "es" | "spa" => Some("spa"),
        "fr" | "fra" => Some("fra"),
        "de" | "deu" => Some("deu"),
        "pt" | "pt-br" | "por" => Some("por"),
        "ru" | "rus" => Some("rus"),
        "it" | "ita" => Some("ita"),
        "ar" | "ara" => Some("ara"),
        _ => None,
    };
    if let Some(code) = mapped {
        return code.to_string();
    }
    if normalized.chars().count() == 3 {
        return normalized;
    }
    let base = normalized.split('-').next().unwrap_or("und");
    if base.chars().count() == 2 {
        let last = base.chars().last().unwrap_or('x');
        return format!("{}{}{}", base, last, last)
            .chars()
            .take(3)
            .collect();
    }
    "und".to_string()
}

fn sanitize_lang_code_for_filename(value: Option<&str>) -> String {
    let cleaned: String = value
        .map(normalize_language_key)
        .unwrap_or_default()
        .chars()
        .filter(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == '-')
        .collect();

    if cleaned.is_empty() {
        "und".to_string()
    } else {
        cleaned
    }
}

fn select_subtitle_format(output_format: &str, codec: &str) -> String {
    if (output_format.is_empty() || output_format == "ass")
        && (codec.contains("ass") || codec.contains("ssa"))
    {
        "ass".to_string()
    } else if output_format == "srt" {
        "srt".to_string()
    } else if output_format == "vtt" {
        "vtt".to_string()
    } else if codec.contains("ass") || codec.contains("ssa") {
        "ass".to_string()
    } else if codec.contains("subrip") || codec.contains("srt") {
        "srt".to_string()
    } else if codec.contains("webvtt") {
        "vtt".to_string()
    } else {
        "srt".to_string()
    }
}

fn persistent_output_path(
    video_path: &str,
    output_directory: Option<&str>,
    lang_code: &str,
    track_index: u32,
    format: &str,
) -> String {
    let video_pathbuf = Path::new(video_path);
    let stem = video_pathbuf
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "subtitle".to_string());
    let timestamp = chrono::Local::now().format("%Y%m%dT%H%M%S");
    let filename = format!(
        "{}_{}_{}_track{}.{}",
        stem, lang_code, timestamp, track_index, format
    );

    if let Some(dir) = output_directory.filter(|d| !d.is_empty()) {
        PathBuf::from(dir)
            .join(filename)
            .to_string_lossy()
            .to_string()
    } else {
        let parent = video_pathbuf.parent().unwrap_or(Path::new("."));
        parent.join(filename).to_string_lossy().to_string()
    }
}

async fn cleanup_generated_file(file_path: Option<&str>) {
    if let Some(file_path) = file_path {
        let path = Path::new(file_path);
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
}

#[tauri::command]
pub async fn start_translation_job(
    app: AppHandle,
    request: TranslationJobRequest,
) -> Result<TranslationJobResult, String> {
    let total_files = request.video_paths.len();
    let mut failures = Vec::new();
    let mut outputs = Vec::new();
    let mut completed_files = 0usize;

    if total_files == 0 {
        return Err("No video files selected".to_string());
    }

    for (file_idx, video_path) in request.video_paths.iter().enumerate() {
        let current_file = file_idx + 1;
        let filename = Path::new(video_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| video_path.clone());
        let file_base_progress = (file_idx as f64 / total_files as f64) * 100.0;
        let file_progress_span = 100.0 / total_files as f64;
        let progress = |step: f64| file_base_progress + (step * file_progress_span);

        emit_job_progress(
            &app,
            current_file,
            total_files,
            progress(0.0),
            format!("Processing {} ({}/{})", filename, current_file, total_files),
        );

        let use_temporary_files = request.embed_subtitles;
        let mut extracted_path: Option<String> = None;
        let mut translated_subtitle_path: Option<String> = None;

        let file_result: Result<TranslationJobOutput, String> = async {
            let video_info =
                super::video::get_video_info(video_path.clone(), request.ffmpeg_path.clone())
                    .await?;

            let track_index = request.subtitle_track.unwrap_or(0);
            let track = video_info
                .subtitle_tracks
                .get(track_index as usize)
                .ok_or_else(|| format!("Track {} not found", track_index))?;

            let format = select_subtitle_format(&request.output_format, &track.codec);

            emit_job_progress(
                &app,
                current_file,
                total_files,
                progress(0.05),
                format!("Extracting subtitles from {}...", filename),
            );

            let extract_result = super::subtitle::extract_subtitle(
                video_path.clone(),
                track_index,
                None,
                Some(format.clone()),
                Some(use_temporary_files),
                request.ffmpeg_path.clone(),
            )
            .await?;

            if !extract_result.success {
                return Err(extract_result
                    .error
                    .unwrap_or_else(|| "Failed to extract subtitle track".to_string()));
            }

            let extracted = extract_result
                .output_path
                .ok_or_else(|| "Subtitle extraction returned no output path".to_string())?;
            extracted_path = Some(extracted.clone());

            emit_job_progress(
                &app,
                current_file,
                total_files,
                progress(0.10),
                format!("Parsing subtitles from {}...", filename),
            );

            let subtitle_data = super::subtitle::parse_subtitle_file(extracted.clone()).await?;
            if subtitle_data.lines.is_empty() {
                return Err("No dialog lines found in extracted subtitle".to_string());
            }

            emit_job_progress(
                &app,
                current_file,
                total_files,
                progress(0.20),
                format!(
                    "Translating {} ({} lines)...",
                    filename,
                    subtitle_data.lines.len()
                ),
            );

            let translated_data = translate_subtitles(
                app.clone(),
                subtitle_data,
                request.config.clone(),
                if request.source_lang.is_empty() {
                    "auto".to_string()
                } else {
                    request.source_lang.clone()
                },
                request.target_lang.clone(),
                Some(request.batch_size.max(1)),
                Some(request.concurrency),
                Some(request.request_delay),
            )
            .await?;

            let target_lang_value = if request.target_lang.is_empty() {
                track.language.as_deref().unwrap_or("und")
            } else {
                request.target_lang.as_str()
            };
            let filename_lang_code = sanitize_lang_code_for_filename(Some(target_lang_value));
            let ffmpeg_lang_code = to_ffmpeg_lang_code(Some(target_lang_value));
            let persistent_path =
                persistent_output_path(video_path, None, &filename_lang_code, track_index, &format);

            emit_job_progress(
                &app,
                current_file,
                total_files,
                progress(0.80),
                format!("Saving translated subtitles for {}...", filename),
            );

            let save_result = save_translated_subtitles(
                translated_data,
                if use_temporary_files {
                    None
                } else {
                    Some(persistent_path)
                },
                extracted_path.clone(),
                Some(use_temporary_files),
            )
            .await?;

            if !save_result.success {
                return Err(save_result.message);
            }

            let saved_subtitle = save_result
                .data
                .ok_or_else(|| "Save returned no subtitle path".to_string())?;
            translated_subtitle_path = Some(saved_subtitle.clone());

            if request.embed_subtitles {
                emit_job_progress(
                    &app,
                    current_file,
                    total_files,
                    progress(0.90),
                    format!("Embedding translated subtitles in {}...", filename),
                );

                let current_info =
                    super::video::get_video_info(video_path.clone(), request.ffmpeg_path.clone())
                        .await?;
                let translated_title = format!("Translated ({})", filename_lang_code);
                let mut tracks_to_remove: Vec<u32> = current_info
                    .subtitle_tracks
                    .iter()
                    .filter(|t| {
                        t.title.as_deref() == Some(translated_title.as_str())
                            || t.title
                                .as_deref()
                                .map(|title| title.starts_with("Translated ("))
                                .unwrap_or(false)
                            || (to_ffmpeg_lang_code(t.language.as_deref()) == ffmpeg_lang_code
                                && t.index != track_index)
                    })
                    .map(|t| t.index)
                    .collect();
                tracks_to_remove.sort_by(|a, b| b.cmp(a));

                for track_to_remove in tracks_to_remove {
                    let remove_result = super::embedding::remove_subtitle_track(
                        video_path.clone(),
                        track_to_remove,
                        request.ffmpeg_path.clone(),
                    )
                    .await?;
                    if !remove_result.success {
                        return Err(remove_result.message);
                    }
                }

                let embed_result = super::embedding::embed_subtitle(
                    video_path.clone(),
                    saved_subtitle,
                    Some(ffmpeg_lang_code),
                    Some(translated_title),
                    true,
                    request.ffmpeg_path.clone(),
                    Some(request.use_mkvmerge),
                )
                .await?;

                if !embed_result.success {
                    return Err(embed_result.message);
                }
            }

            Ok(TranslationJobOutput {
                video_path: video_path.clone(),
                subtitle_path: if request.embed_subtitles {
                    None
                } else {
                    translated_subtitle_path.clone()
                },
                embedded: request.embed_subtitles,
            })
        }
        .await;

        if use_temporary_files {
            cleanup_generated_file(extracted_path.as_deref()).await;
            cleanup_generated_file(translated_subtitle_path.as_deref()).await;
        }

        match file_result {
            Ok(output) => {
                completed_files += 1;
                outputs.push(output);
                emit_job_progress(
                    &app,
                    current_file,
                    total_files,
                    progress(1.0),
                    format!("Finished {}", filename),
                );
            }
            Err(reason) => {
                let failure = format!("{}: {}", filename, reason);
                eprintln!("{}", failure);
                emit_job_progress(
                    &app,
                    current_file,
                    total_files,
                    progress(1.0),
                    format!("Error in {}: {}", filename, reason),
                );
                failures.push(failure);
            }
        }
    }

    let status = if failures.is_empty() {
        "Translation complete!".to_string()
    } else if completed_files == 0 {
        format!("Translation failed: {}", failures[0])
    } else {
        format!(
            "Translation finished with errors ({}/{}): {}",
            completed_files, total_files, failures[0]
        )
    };
    emit_job_progress(&app, total_files, total_files, 100.0, status);

    Ok(TranslationJobResult {
        completed_files,
        total_files,
        failures,
        outputs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(index: usize, text: &str, original: &str, start: &str, end: &str) -> DialogLine {
        DialogLine {
            index,
            text: text.to_string(),
            original_with_formatting: original.to_string(),
            start: start.to_string(),
            end: end.to_string(),
            style: Some("Default".to_string()),
            name: None,
        }
    }

    #[test]
    fn reconstruct_srt_writes_ordered_blocks() {
        let lines = vec![
            line(0, "Olá", "Hello", "00:00:01,000", "00:00:02,000"),
            line(1, "Mundo", "World", "00:00:03,000", "00:00:04,000"),
        ];

        let output = reconstruct_srt(&lines);

        assert!(output.contains("1\n00:00:01,000 --> 00:00:02,000\nOlá"));
        assert!(output.contains("2\n00:00:03,000 --> 00:00:04,000\nMundo"));
    }

    #[test]
    fn reconstruct_vtt_writes_webvtt_header() {
        let lines = vec![line(0, "Bonjour", "Hello", "00:00:01.000", "00:00:02.000")];

        let output = reconstruct_vtt(&lines);

        assert!(output.starts_with("WEBVTT\n\n"));
        assert!(output.contains("00:00:01.000 --> 00:00:02.000\nBonjour"));
    }

    #[test]
    fn reconstruct_ass_replaces_dialogue_and_preserves_leading_tags() {
        let original = r#"[Script Info]
Title: Example

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, Encoding
Style: Default,Arial,20,&H00FFFFFF,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\i1}Hello
Dialogue: 0,0:00:03.00,0:00:04.00,Signs,,0,0,0,,Shop sign
"#;
        let lines = vec![line(0, "Olá", "{\\i1}Hello", "0:00:01.00", "0:00:02.00")];

        let output = reconstruct_ass(original, &lines);

        assert!(output.contains("Style: Default,Arial,20,&H00FFFFFF,0"));
        assert!(output.contains("Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\\i1}Olá"));
        assert!(output.contains("Shop sign"));
    }

    #[test]
    fn helper_maps_language_codes_for_embedding_and_filenames() {
        assert_eq!(to_ffmpeg_lang_code(Some("pt-BR")), "por");
        assert_eq!(to_ffmpeg_lang_code(Some("en_US")), "eng");
        assert_eq!(to_ffmpeg_lang_code(Some("xx")), "xxx");
        assert_eq!(to_ffmpeg_lang_code(None), "und");
        assert_eq!(sanitize_lang_code_for_filename(Some("pt_BR!")), "pt-br");
        assert_eq!(sanitize_lang_code_for_filename(None), "und");
    }

    #[test]
    fn helper_selects_subtitle_format_from_setting_or_codec() {
        assert_eq!(select_subtitle_format("ass", "ass"), "ass");
        assert_eq!(select_subtitle_format("srt", "ass"), "srt");
        assert_eq!(select_subtitle_format("", "webvtt"), "vtt");
        assert_eq!(select_subtitle_format("", "unknown"), "srt");
    }

    #[test]
    fn helper_places_persistent_output_in_custom_directory() {
        let path = persistent_output_path(
            "/videos/Episode 01.mkv",
            Some("/tmp/animesubs-out"),
            "por",
            2,
            "srt",
        );

        assert!(path.starts_with("/tmp/animesubs-out/"));
        assert!(path.contains("Episode 01_por_"));
        assert!(path.ends_with("_track2.srt"));
    }
}
