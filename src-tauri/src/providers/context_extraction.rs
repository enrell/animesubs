use crate::models::{
    ContextExtractionRequest, ContextProfile, MediaMatch
};
use crate::providers::anime_tools;
use crate::providers::{
    build_gemini_generate_content_endpoint, extract_response_content, ResponseFormat,
};
use regex::Regex;
use reqwest::Client;

fn build_subtitle_sample(lines: &[crate::models::DialogLine], max_lines: usize) -> String {
    let sample: Vec<&crate::models::DialogLine> =
        lines.iter().take(max_lines).collect();
    sample
        .iter()
        .map(|line| {
            let speaker = line.name.as_deref().unwrap_or("").trim();
            if speaker.is_empty() {
                line.text.clone()
            } else {
                format!("{}: {}", speaker, line.text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_classifier_prompt(
    filename: &str,
    source_lang: &str,
    subtitle_sample: &str,
) -> String {
    format!(
        "You are an anime title classifier.\n\
         Given a video filename and a sample of subtitle dialogue, your \
         ONLY task is to identify the anime title so it can be searched in \
         a database.\n\n\
         Rules:\n\
         - Extract the clean anime title from the filename (remove episode \
         numbers, quality tags, group names, file extensions, etc).\n\
         - If the filename is not clear enough, use the subtitle dialogue to \
         infer the title.\n\
         - Return ONLY the anime title in its original language (romaji or \
         English). No explanations, no JSON, no extra text.\n\
         - If you cannot determine the title, return the cleaned filename \
         stem as-is.\n\n\
         Filename: {}\n\
         Subtitle language: {}\n\
         Subtitle sample (first lines):\n{}\n\n\
         Respond with ONLY the anime title, nothing else.",
        filename, source_lang, subtitle_sample
    )
}

fn clean_title(raw: &str) -> String {
    let thinking_regex =
        Regex::new(r"(?is)<(?:thinking|think)>.*?</(?:thinking|think)>").unwrap();
    let cleaned = thinking_regex.replace_all(raw, "").to_string();
    let cleaned = cleaned.trim();
    let cleaned = cleaned.trim_matches(|c: char| c == '"' || c == '\'' || c == '`');
    cleaned
        .lines()
        .next()
        .unwrap_or(cleaned)
        .trim()
        .to_string()
}

async fn classify_anime_title(
    config: &crate::models::LLMConfig,
    filename: &str,
    source_lang: &str,
    subtitle_sample: &str,
) -> Result<String, String> {
    let prompt = build_classifier_prompt(filename, source_lang, subtitle_sample);
    let client = Client::new();
    let provider = config.provider.trim().to_ascii_lowercase();
    let is_gemini_openai_compat =
        provider == "gemini" && config.endpoint.contains("/openai");
    let uses_ollama_native_api =
        provider == "ollama" && !config.endpoint.contains("/v1");
    let is_openai_compatible = matches!(
        provider.as_str(),
        "openai" | "openrouter" | "custom" | "minimax" | "nvidia" | "lmstudio"
            | "llamacpp"
    ) || is_gemini_openai_compat
        || (provider == "ollama" && !uses_ollama_native_api);

    let (body, endpoint_url, response_format) = if is_openai_compatible {
        let base = config.endpoint.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        };
        (
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": "You are a title classifier. Respond with ONLY the title, no extra text."},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.1,
                "max_tokens": 100
            }),
            url,
            ResponseFormat::OpenAiCompatible,
        )
    } else if provider == "gemini" {
        (
            serde_json::json!({
                "contents": [{
                    "parts": [{"text": format!(
                        "You are a title classifier. Respond with ONLY the title, no extra text.\n\n{}",
                        prompt
                    )}]
                }],
                "generationConfig": {
                    "temperature": 0.1,
                    "maxOutputTokens": 100
                }
            }),
            build_gemini_generate_content_endpoint(
                &config.endpoint,
                &config.model,
                &config.api_key,
            ),
            ResponseFormat::Gemini,
        )
    } else if uses_ollama_native_api {
        let base = config.endpoint.trim_end_matches('/');
        let url = if base.ends_with("/api/chat") {
            base.to_string()
        } else if base.ends_with("/api") {
            format!("{}/chat", base)
        } else {
            format!("{}/api/chat", base)
        };
        (
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": "You are a title classifier. Respond with ONLY the title, no extra text."},
                    {"role": "user", "content": prompt}
                ],
                "stream": false,
                "options": { "temperature": 0.1 }
            }),
            url,
            ResponseFormat::OllamaNative,
        )
    } else {
        return Err(format!("Unsupported provider: {}", config.provider));
    };

    let mut req = client.post(&endpoint_url).json(&body);

    if is_gemini_openai_compat {
        req = req.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match provider.as_str() {
            "openai" | "openrouter" | "custom" | "minimax" | "nvidia"
                if !config.api_key.is_empty() =>
            {
                req = req.header("Authorization", format!("Bearer {}", config.api_key));
                if provider == "openrouter" {
                    req = req.header("HTTP-Referer", "https://animesubs.app");
                }
            }
            _ => {}
        }
    }

    eprintln!("Phase 1: classifying anime title from filename");
    let response = req
        .send()
        .await
        .map_err(|e| format!("Classifier API call failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Classifier API error ({}): {}", status, error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse classifier response: {}", e))?;

    let content = extract_response_content(&response_json, response_format)?;
    let title = clean_title(&content);
    eprintln!("Phase 1 result: classified title = '{}'", title);

    if title.is_empty() {
        return Err("Classifier returned an empty title".to_string());
    }

    Ok(title)
}

fn pick_best_match<'a>(
    matches: &'a [MediaMatch],
    classified_title: &str,
) -> Option<&'a MediaMatch> {
    if matches.is_empty() {
        return None;
    }

    let title_lower = classified_title.to_lowercase();
    for m in matches {
        if m.title_display.to_lowercase() == title_lower {
            return Some(m);
        }
    }

    for m in matches {
        if m
            .title_english
            .as_ref()
            .map(|t| t.to_lowercase() == title_lower)
            .unwrap_or(false)
        {
            return Some(m);
        }
    }

    for m in matches {
        if m
            .title_romaji
            .as_ref()
            .map(|t| t.to_lowercase() == title_lower)
            .unwrap_or(false)
        {
            return Some(m);
        }
    }

    for m in matches {
        if m
            .aliases
            .iter()
            .any(|a| a.to_lowercase() == title_lower)
        {
            return Some(m);
        }
    }

    for m in matches {
        if m.title_display.to_lowercase().contains(&title_lower)
            || title_lower.contains(&m.title_display.to_lowercase())
        {
            return Some(m);
        }
    }

    Some(&matches[0])
}

fn build_profile_from_media(
    media: &MediaMatch,
    source_path: Option<String>,
) -> ContextProfile {
    let now = chrono::Local::now().to_rfc3339();

    let title = media
        .title_english
        .clone()
        .or_else(|| media.title_romaji.clone())
        .or_else(|| Some(media.title_display.clone()));

    ContextProfile {
        title,
        characters: Vec::new(),
        locations: Vec::new(),
        plot_summary: media.synopsis.clone(),
        terminology: Vec::new(),
        tone_notes: None,
        custom_instructions: None,
        source_path,
        created_at: now.clone(),
        updated_at: now,
        media_match: Some(media.clone()),
    }
}

#[derive(serde::Deserialize)]
struct LLMExtractionPayload {
    characters: Option<Vec<CharacterPayload>>,
    locations: Option<Vec<String>>,
    terminology: Option<Vec<TerminologyPayload>>,
    tone_notes: Option<String>,
    plot_summary: Option<String>,
}

#[derive(serde::Deserialize)]
struct CharacterPayload {
    name: String,
    role: Option<String>,
    description: Option<String>,
    relationships: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct TerminologyPayload {
    source_term: String,
    translated_term: String,
    notes: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn perform_llm_extraction(
    config: &crate::models::LLMConfig,
    title: &str,
    synopsis: Option<&str>,
    verified_chars: &[crate::models::CharacterProfile],
    source_lang: &str,
    target_lang: &str,
    subtitle_sample: &str,
    custom_instructions: Option<&str>,
) -> Result<LLMExtractionPayload, String> {
    let verified_str = if verified_chars.is_empty() {
        "None".to_string()
    } else {
        verified_chars
            .iter()
            .map(|c| format!("- {} ({})", c.name, c.role.as_deref().unwrap_or("unknown")))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let prompt = format!(
        "You are an anime story and subtitle analyst.\n\
         Analyze the provided anime info and subtitle dialogue to extract a structured context profile for translation from {} to {}.\n\n\
         Anime Title: {}\n\
         Synopsis: {}\n\
         Verified Characters from Database:\n{}\n\n\
         Subtitle Dialogue Sample:\n{}\n\n\
         Custom Instructions: {}\n\n\
         Return ONLY a valid JSON object matching this schema (no markdown formatting, no code block backticks):\n\
         {{\n\
           \"characters\": [\n\
             {{\"name\": \"Character Name\", \"role\": \"Role/Archetype\", \"description\": \"Brief description\", \"relationships\": [\"Relationship\"]}}\n\
           ],\n\
           \"locations\": [\"Location 1\"],\n\
           \"terminology\": [\n\
             {{\"source_term\": \"Original Term\", \"translated_term\": \"Translation\", \"notes\": \"Context\"}}\n\
           ],\n\
           \"tone_notes\": \"Summary of tone, formality, honorifics, and speaker styles\",\n\
           \"plot_summary\": \"Brief summary of episode plot/scene\"\n\
         }}",
        source_lang,
        target_lang,
        title,
        synopsis.unwrap_or("N/A"),
        verified_str,
        subtitle_sample,
        custom_instructions.unwrap_or("None")
    );

    let client = Client::new();
    let provider = config.provider.trim().to_ascii_lowercase();
    let is_gemini_openai_compat =
        provider == "gemini" && config.endpoint.contains("/openai");
    let uses_ollama_native_api =
        provider == "ollama" && !config.endpoint.contains("/v1");
    let is_openai_compatible = matches!(
        provider.as_str(),
        "openai" | "openrouter" | "custom" | "minimax" | "nvidia" | "lmstudio"
            | "llamacpp"
    ) || is_gemini_openai_compat
        || (provider == "ollama" && !uses_ollama_native_api);

    let (body, endpoint_url, response_format) = if is_openai_compatible {
        let base = config.endpoint.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        };
        (
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": "You are a JSON context extractor. Respond with ONLY valid JSON."},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.2,
                "max_tokens": 1500
            }),
            url,
            ResponseFormat::OpenAiCompatible,
        )
    } else if provider == "gemini" {
        (
            serde_json::json!({
                "contents": [{
                    "parts": [{"text": format!(
                        "You are a JSON context extractor. Respond with ONLY valid JSON.\n\n{}",
                        prompt
                    )}]
                }],
                "generationConfig": {
                    "temperature": 0.2,
                    "maxOutputTokens": 1500
                }
            }),
            build_gemini_generate_content_endpoint(
                &config.endpoint,
                &config.model,
                &config.api_key,
            ),
            ResponseFormat::Gemini,
        )
    } else if uses_ollama_native_api {
        let base = config.endpoint.trim_end_matches('/');
        let url = if base.ends_with("/api/chat") {
            base.to_string()
        } else if base.ends_with("/api") {
            format!("{}/chat", base)
        } else {
            format!("{}/api/chat", base)
        };
        (
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": "You are a JSON context extractor. Respond with ONLY valid JSON."},
                    {"role": "user", "content": prompt}
                ],
                "stream": false,
                "options": { "temperature": 0.2 }
            }),
            url,
            ResponseFormat::OllamaNative,
        )
    } else {
        return Err(format!("Unsupported provider: {}", config.provider));
    };

    let mut req = client.post(&endpoint_url).json(&body);

    if is_gemini_openai_compat {
        req = req.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match provider.as_str() {
            "openai" | "openrouter" | "custom" | "minimax" | "nvidia"
                if !config.api_key.is_empty() =>
            {
                req = req.header("Authorization", format!("Bearer {}", config.api_key));
                if provider == "openrouter" {
                    req = req.header("HTTP-Referer", "https://animesubs.app");
                }
            }
            _ => {}
        }
    }

    let response = req
        .send()
        .await
        .map_err(|e| format!("LLM extraction call failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("LLM extraction error ({}): {}", status, error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse response JSON: {}", e))?;

    let content = extract_response_content(&response_json, response_format)?;
    let payload = parse_llm_json(&content)?;
    Ok(payload)
}

fn parse_llm_json<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, String> {
    let thinking_regex = Regex::new(r"(?is)<(?:thinking|think)>.*?</(?:thinking|think)>").unwrap();
    let cleaned = thinking_regex.replace_all(raw, "").to_string();

    let json_str = if let Some(start) = cleaned.find('{') {
        if let Some(end) = cleaned.rfind('}') {
            &cleaned[start..=end]
        } else {
            cleaned.trim()
        }
    } else {
        cleaned.trim()
    };

    serde_json::from_str(json_str).map_err(|e| format!("Failed to parse JSON: {} from raw output: {}", e, raw))
}

pub async fn extract_context(
    request: &ContextExtractionRequest,
) -> Result<ContextProfile, String> {
    let filename = request.filename.trim().to_string();

    if filename.is_empty() {
        return Err("Filename is required for context extraction".to_string());
    }

    let sample = build_subtitle_sample(&request.subtitle_data.lines, 30);

    let classified_title = classify_anime_title(
        &request.config,
        &filename,
        &request.source_lang,
        &sample,
    )
    .await?;

    eprintln!("Phase 2: searching metadata providers for '{}'", classified_title);
    let search_query = classified_title.clone();
    let tmdb_key = request.tmdb_api_key.clone();
    let search_results = tokio::task::spawn_blocking(move || {
        anime_tools::search_anime_combined(&search_query, 5, tmdb_key.as_deref())
    })
    .await
    .map_err(|e| format!("Search task failed: {}", e))??;

    if search_results.is_empty() {
        eprintln!("Phase 2: no metadata results found");
        let fallback_media = MediaMatch {
            provider: "AniList".to_string(),
            source_id: String::new(),
            title_display: classified_title.clone(),
            title_native: None,
            title_english: Some(classified_title.clone()),
            title_romaji: None,
            synopsis: None,
            genres: Vec::new(),
            tags: Vec::new(),
            aliases: Vec::new(),
            format: None,
            episodes: None,
            season: None,
            season_year: None,
        };

        let sample_60 = build_subtitle_sample(&request.subtitle_data.lines, 60);
        let llm_res = perform_llm_extraction(
            &request.config,
            &classified_title,
            None,
            &[],
            &request.source_lang,
            &request.target_lang,
            &sample_60,
            request.custom_instructions.as_deref(),
        )
        .await;

        let mut profile = build_profile_from_media(&fallback_media, Some(request.subtitle_data.source_path.clone()));
        if let Ok(llm_data) = llm_res {
            if let Some(extracted_chars) = llm_data.characters {
                profile.characters = extracted_chars
                    .into_iter()
                    .map(|c| crate::models::CharacterProfile {
                        name: c.name,
                        role: c.role,
                        description: c.description,
                        relationships: c.relationships,
                        verified: false,
                    })
                    .collect();
            }
            if let Some(locs) = llm_data.locations {
                profile.locations = locs;
            }
            if let Some(terms) = llm_data.terminology {
                profile.terminology = terms
                    .into_iter()
                    .map(|t| crate::models::TerminologyEntry {
                        source_term: t.source_term,
                        translated_term: t.translated_term,
                        notes: t.notes,
                    })
                    .collect();
            }
            if let Some(tone) = llm_data.tone_notes {
                profile.tone_notes = Some(tone);
            }
            if let Some(plot) = llm_data.plot_summary {
                profile.plot_summary = Some(plot);
            }
        }

        if let Some(custom) = request.custom_instructions.as_deref() {
            profile.custom_instructions = Some(custom.to_string());
        }

        return Ok(profile);
    }

    let best = pick_best_match(&search_results, &classified_title);
    let best = best.ok_or("No suitable metadata match found")?;

    eprintln!(
        "Phase 2: best match = '{}' (Provider: {}, ID: {})",
        best.title_display, best.provider, best.source_id
    );

    eprintln!("Phase 3: fetching full metadata by ID");
    let best_id = best.source_id.clone();
    let provider_name = best.provider.clone();
    let tmdb_key_for_id = request.tmdb_api_key.clone();
    let full_media = if best_id.is_empty() {
        None
    } else {
        tokio::task::spawn_blocking(move || {
            anime_tools::get_anime_by_id_with_provider(&provider_name, &best_id, tmdb_key_for_id.as_deref())
        })
        .await
        .map_err(|e| format!("Metadata lookup task failed: {}", e))?
        .ok()
        .flatten()
    };

    let media = full_media.as_ref().unwrap_or(best);
    eprintln!(
        "Phase 3: retrieved metadata for '{}' (genres: {})",
        media.title_display,
        media.genres.join(", ")
    );

    let mut verified_chars = if !media.source_id.is_empty() {
        anime_tools::fetch_anilist_characters(&media.source_id).await
    } else {
        Vec::new()
    };

    eprintln!("Phase 4: extracting characters, terminology, and tone via LLM");
    let sample_60 = build_subtitle_sample(&request.subtitle_data.lines, 60);
    let llm_res = perform_llm_extraction(
        &request.config,
        media.title_english.as_deref().unwrap_or(&classified_title),
        media.synopsis.as_deref(),
        &verified_chars,
        &request.source_lang,
        &request.target_lang,
        &sample_60,
        request.custom_instructions.as_deref(),
    )
    .await;

    let mut profile = build_profile_from_media(media, Some(request.subtitle_data.source_path.clone()));

    match llm_res {
        Ok(llm_data) => {
            if let Some(extracted_chars) = llm_data.characters {
                for ex_char in extracted_chars {
                    if ex_char.name.trim().is_empty() {
                        continue;
                    }
                    if let Some(v_char) = verified_chars
                        .iter_mut()
                        .find(|vc| vc.name.eq_ignore_ascii_case(&ex_char.name))
                    {
                        if v_char.description.is_none() && ex_char.description.is_some() {
                            v_char.description = ex_char.description;
                        }
                        if v_char.role.is_none() && ex_char.role.is_some() {
                            v_char.role = ex_char.role;
                        }
                        if ex_char.relationships.is_some() {
                            v_char.relationships = ex_char.relationships;
                        }
                    } else {
                        verified_chars.push(crate::models::CharacterProfile {
                            name: ex_char.name,
                            role: ex_char.role,
                            description: ex_char.description,
                            relationships: ex_char.relationships,
                            verified: false,
                        });
                    }
                }
            }

            profile.characters = verified_chars;

            if let Some(locs) = llm_data.locations {
                profile.locations = locs;
            }
            if let Some(terms) = llm_data.terminology {
                profile.terminology = terms
                    .into_iter()
                    .map(|t| crate::models::TerminologyEntry {
                        source_term: t.source_term,
                        translated_term: t.translated_term,
                        notes: t.notes,
                    })
                    .collect();
            }
            if let Some(tone) = llm_data.tone_notes {
                profile.tone_notes = Some(tone);
            }
            if let Some(plot) = llm_data.plot_summary {
                profile.plot_summary = Some(plot);
            }
        }
        Err(e) => {
            eprintln!("LLM context extraction phase failed (non-fatal, using AniList characters): {}", e);
            profile.characters = verified_chars;
        }
    }

    if let Some(custom) = request.custom_instructions.as_deref() {
        profile.custom_instructions = Some(custom.to_string());
    }

    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_title_strips_thinking_tags() {
        let raw = "<thinking>Let me analyze...</thinking>Frieren: Beyond Journey's End";
        assert_eq!(clean_title(raw), "Frieren: Beyond Journey's End");
    }

    #[test]
    fn clean_title_strips_quotes() {
        assert_eq!(clean_title("\"Sousou no Frieren\""), "Sousou no Frieren");
    }

    #[test]
    fn clean_title_takes_first_line() {
        assert_eq!(clean_title("Monster\n\nExtra text"), "Monster");
    }

    #[test]
    fn pick_best_match_exact_display_title() {
        let matches = vec![sample_media("Frieren", "1"), sample_media("Monster", "2")];
        let best = pick_best_match(&matches, "Frieren");
        assert_eq!(best.unwrap().source_id, "1");
    }

    #[test]
    fn pick_best_match_english_title() {
        let mut m = sample_media("Sousou no Frieren", "1");
        m.title_english = Some("Frieren: Beyond Journey's End".to_string());
        let matches = vec![m];
        let best = pick_best_match(&matches, "Frieren: Beyond Journey's End");
        assert_eq!(best.unwrap().source_id, "1");
    }

    #[test]
    fn pick_best_match_falls_back_to_first() {
        let matches = vec![sample_media("Random Anime", "1"), sample_media("Other", "2")];
        let best = pick_best_match(&matches, "Something Completely Different");
        assert_eq!(best.unwrap().source_id, "1");
    }

    #[test]
    fn pick_best_match_empty_returns_none() {
        let matches: Vec<MediaMatch> = vec![];
        assert!(pick_best_match(&matches, "Anything").is_none());
    }

    #[test]
    fn build_profile_from_media_populates_fields() {
        let media = sample_media("Test Anime", "123");
        let profile = build_profile_from_media(&media, Some("/path/to/video.mkv".to_string()));

        assert_eq!(profile.title.as_deref(), Some("Test Anime"));
        assert!(profile.media_match.is_some());
        assert_eq!(profile.media_match.as_ref().unwrap().source_id, "123");
        assert_eq!(profile.source_path.as_deref(), Some("/path/to/video.mkv"));
    }

    fn sample_media(title: &str, id: &str) -> MediaMatch {
        MediaMatch {
            provider: "AniList".to_string(),
            source_id: id.to_string(),
            title_display: title.to_string(),
            title_native: None,
            title_english: Some(title.to_string()),
            title_romaji: None,
            synopsis: Some("A test synopsis.".to_string()),
            genres: vec!["Action".to_string()],
            tags: vec![],
            aliases: vec![],
            format: Some("TV".to_string()),
            episodes: Some(12),
            season: Some("fall".to_string()),
            season_year: Some(2024),
        }
    }
}
