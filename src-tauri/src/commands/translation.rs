use crate::models::*;
use crate::utils::*;
use futures::future::join_all;
use reqwest::Client;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tokio::sync::Mutex;

async fn call_llm_api(
    config: &LLMConfig,
    lines: &[TranslationLine],
    source_lang: &str,
    target_lang: &str,
) -> Result<Vec<TranslatedLine>, String> {
    let client = Client::new();

    let system_prompt = build_translation_prompt(&config.system_prompt, source_lang, target_lang);

    let user_content = serde_json::json!({
        "lines": lines
    });

    let is_gemini_openai_compat =
        config.provider == "gemini" && config.endpoint.contains("/openai");

    let request_body = match config.provider.as_str() {
        "openai" | "openrouter" | "minimax" | _ if is_gemini_openai_compat => {
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

    let endpoint_url = if is_gemini_openai_compat {
        let base = config.endpoint.trim_end_matches('/');
        format!("{}/chat/completions", base)
    } else if config.provider == "gemini" {
        format!("{}:generateContent?key={}", config.endpoint, config.api_key)
    } else {
        let base = config.endpoint.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        }
    };

    let mut request = client.post(&endpoint_url).json(&request_body);

    if is_gemini_openai_compat {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match config.provider.as_str() {
            "openai" | "openrouter" | "minimax" => {
                request = request.header("Authorization", format!("Bearer {}", config.api_key));
                if config.provider == "openrouter" {
                    request = request.header("HTTP-Referer", "https://animesubs.app");
                }
            }
            "gemini" => {}
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

    let content = if is_gemini_openai_compat
        || config.provider == "openai"
        || config.provider == "openrouter"
        || config.provider == "minimax"
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

    // Remove reasoning/thinking blocks from models like DeepSeek R1, OpenAI o1, etc.
    let thinking_regex = Regex::new(r"<thinking>.*?</thinking>").unwrap();
    let content_without_thinking = thinking_regex.replace_all(&content, "").to_string();

    let cleaned_content = clean_json_response(&content_without_thinking);

    let translation_response: TranslationResponse = serde_json::from_str(&cleaned_content)
        .map_err(|e| {
            format!(
                "Failed to parse translation JSON: {}. Response was: {}",
                e, cleaned_content
            )
        })?;

    Ok(translation_response.translations)
}

#[tauri::command]
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
    let concurrency = concurrency.unwrap_or(1).max(1).min(10);
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