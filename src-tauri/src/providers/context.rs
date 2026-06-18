use crate::models::{LLMConfig, TranslatedLine, TranslationLine};
use crate::utils::build_translation_prompt;
use regex::Regex;
use reqwest::Client;

use super::{
    build_gemini_generate_content_endpoint, extract_response_content, parse_translation_response_content,
    ProviderRequest, ResponseFormat,
};

/// Builds a provider request with optional compacted context from previous chunks.
pub(crate) fn build_provider_request_with_context(
    config: &LLMConfig,
    lines: &[TranslationLine],
    source_lang: &str,
    target_lang: &str,
    compact_context: Option<&str>,
) -> Result<ProviderRequest, String> {
    let mut system_prompt = build_translation_prompt(&config.system_prompt, source_lang, target_lang);
    if let Some(ctx) = compact_context.filter(|c| !c.trim().is_empty()) {
        system_prompt = format!(
            "{}\n\nCONTEXT FROM PREVIOUS SUBTITLES (characters, plot, terminology):\n{}",
            system_prompt, ctx
        );
    }
    let user_content = serde_json::json!({ "lines": lines });
    let provider = config.provider.trim().to_ascii_lowercase();
    let is_gemini_openai_compat = provider == "gemini" && config.endpoint.contains("/openai");
    let uses_ollama_native_api = provider == "ollama" && !config.endpoint.contains("/v1");
    let is_openai_compatible = matches!(
        provider.as_str(),
        "openai" | "openrouter" | "custom" | "minimax" | "nvidia" | "lmstudio" | "llamacpp"
    ) || is_gemini_openai_compat
        || (provider == "ollama" && !uses_ollama_native_api);

    if is_openai_compatible {
        let base = config.endpoint.trim_end_matches('/');
        let endpoint_url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        };

        return Ok(ProviderRequest {
            body: serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content.to_string()}
                ],
                "temperature": 0.3,
                "response_format": {"type": "json_object"}
            }),
            endpoint_url,
            response_format: ResponseFormat::OpenAiCompatible,
            provider,
            is_gemini_openai_compat,
        });
    }

    if provider == "gemini" {
        return Ok(ProviderRequest {
            body: serde_json::json!({
                "contents": [{
                    "parts": [{
                        "text": format!(
                            "{}\n\nTranslate the following:\n{}",
                            system_prompt, user_content
                        )
                    }]
                }],
                "generationConfig": {
                    "temperature": 0.3,
                    "responseMimeType": "application/json"
                }
            }),
            endpoint_url: build_gemini_generate_content_endpoint(
                &config.endpoint,
                &config.model,
                &config.api_key,
            ),
            response_format: ResponseFormat::Gemini,
            provider,
            is_gemini_openai_compat,
        });
    }

    if uses_ollama_native_api {
        let base = config.endpoint.trim_end_matches('/');
        let endpoint_url = if base.ends_with("/api/chat") {
            base.to_string()
        } else if base.ends_with("/api") {
            format!("{}/chat", base)
        } else {
            format!("{}/api/chat", base)
        };

        return Ok(ProviderRequest {
            body: serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": user_content.to_string()}
                ],
                "stream": false,
                "format": "json",
                "options": { "temperature": 0.3 }
            }),
            endpoint_url,
            response_format: ResponseFormat::OllamaNative,
            provider,
            is_gemini_openai_compat,
        });
    }

    Err(format!("Unsupported provider: {}", config.provider))
}

/// Calls the LLM API with optional compacted context from previous translation chunks.
pub async fn call_llm_api_with_context(
    config: &LLMConfig,
    lines: &[TranslationLine],
    source_lang: &str,
    target_lang: &str,
    compact_context: Option<&str>,
) -> Result<Vec<TranslatedLine>, String> {
    let client = Client::new();
    let provider_request = build_provider_request_with_context(
        config,
        lines,
        source_lang,
        target_lang,
        compact_context,
    )?;
    let mut request = client
        .post(&provider_request.endpoint_url)
        .json(&provider_request.body);

    if provider_request.is_gemini_openai_compat {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match provider_request.provider.as_str() {
            "openai" | "openrouter" | "custom" | "minimax" | "nvidia" => {
                if !config.api_key.is_empty() {
                    request =
                        request.header("Authorization", format!("Bearer {}", config.api_key));
                }
                if provider_request.provider == "openrouter" {
                    request = request.header("HTTP-Referer", "https://animesubs.app");
                }
            }
            _ => {}
        }
    }

    eprintln!(
        "Calling LLM API (context-aware): {} with model {}",
        provider_request.endpoint_url, config.model
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

    let content = extract_response_content(&response_json, provider_request.response_format)?;

    eprintln!("LLM response content: {}", content);
    parse_translation_response_content(&content)
}

/// Asks the LLM to produce a compact summary of translated subtitle pairs.
/// Used as context for subsequent translation chunks.
pub async fn generate_compaction_summary(
    config: &LLMConfig,
    translated_pairs: &[String],
    source_lang: &str,
    target_lang: &str,
) -> Result<String, String> {
    let pairs_text = translated_pairs.join("\n");
    let prompt = format!(
        "You are a translation context summarizer.\n\
         Given pairs of source->translated subtitle lines, produce a CONCISE\n\
         context summary (max 300 words) for the next batch of translations.\n\n\
         Include ONLY:\n\
         Character names mentioned and their roles/relationships\n\
         Key plot situation or scene context\n\
         Important terminology choices (honorifics handling, name translations)\n\
         Tone and register used (formal/casual/etc)\n\n\
         Do NOT include:\n\
         Individual line translations\n\
         Generic translation advice\n\
         Repeated information\n\n\
         Source language: {}\n\
         Target language: {}\n\n\
         Translated pairs:\n{}\n\n\
         Respond with ONLY the summary text, no JSON, no formatting.",
        source_lang, target_lang, pairs_text
    );

    let client = Client::new();
    let provider = config.provider.trim().to_ascii_lowercase();
    let is_gemini_openai_compat =
        provider == "gemini" && config.endpoint.contains("/openai");
    let uses_ollama_native_api =
        provider == "ollama" && !config.endpoint.contains("/v1");
    let is_openai_compatible = matches!(
        provider.as_str(),
        "openai" | "openrouter" | "custom" | "minimax" | "nvidia" | "lmstudio" | "llamacpp"
    ) || is_gemini_openai_compat
        || (provider == "ollama" && !uses_ollama_native_api);

    let (endpoint_url, body) = if is_openai_compatible {
        let base = config.endpoint.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{}/chat/completions", base)
        };
        (
            url,
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": "You are a concise summarizer for translation context."},
                    {"role": "user", "content": prompt}
                ],
                "temperature": 0.3,
                "max_tokens": 500
            }),
        )
    } else if provider == "gemini" {
        (
            build_gemini_generate_content_endpoint(
                &config.endpoint,
                &config.model,
                &config.api_key,
            ),
            serde_json::json!({
                "contents": [{
                    "parts": [{"text": prompt}]
                }],
                "generationConfig": {
                    "temperature": 0.3,
                    "maxOutputTokens": 500
                }
            }),
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
            url,
            serde_json::json!({
                "model": config.model,
                "messages": [
                    {"role": "system", "content": "You are a concise summarizer for translation context."},
                    {"role": "user", "content": prompt}
                ],
                "stream": false,
                "options": { "temperature": 0.3 }
            }),
        )
    } else {
        return Err(format!("Unsupported provider: {}", config.provider));
    };

    let mut request = client.post(&endpoint_url).json(&body);

    if is_gemini_openai_compat {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match provider.as_str() {
            "openai" | "openrouter" | "custom" | "minimax" | "nvidia"
                if !config.api_key.is_empty() =>
            {
                request =
                    request.header("Authorization", format!("Bearer {}", config.api_key));
            }
            _ => {}
        }
    }

    let response = request
        .send()
        .await
        .map_err(|e| format!("Compaction request failed: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(format!("Compaction API error ({}): {}", status, error_text));
    }

    let response_json: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse compaction response: {}", e))?;

    let content = if uses_ollama_native_api && !is_gemini_openai_compat {
        response_json["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string()
    } else if provider == "gemini" && !is_gemini_openai_compat {
        extract_response_content(&response_json, ResponseFormat::Gemini).unwrap_or_default()
    } else {
        extract_response_content(&response_json, ResponseFormat::OpenAiCompatible)
            .unwrap_or_default()
    };

    // Strip any thinking tags
    let thinking_regex =
        Regex::new(r"(?is)<(?:thinking|think)>.*?</(?:thinking|think)>").unwrap();
    let cleaned = thinking_regex.replace_all(&content, "").to_string();

    Ok(cleaned.trim().to_string())
}
