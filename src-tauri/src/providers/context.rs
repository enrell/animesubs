use crate::models::{LLMConfig, TranslatedLine, TranslationLine};
use crate::utils::build_translation_prompt;

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
    let mut system_prompt = if config.system_prompt.trim().is_empty() {
        build_translation_prompt(&config.system_prompt, source_lang, target_lang)
    } else {
        config.system_prompt.clone()
    };
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
                "max_tokens": 8192,
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
                    "maxOutputTokens": 8192,
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
                "options": { "temperature": 0.3, "num_predict": 8192 }
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
