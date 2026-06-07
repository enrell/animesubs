use crate::models::{LLMConfig, TranslatedLine, TranslationLine, TranslationResponse};
use crate::utils::{build_translation_prompt, clean_json_response};
use regex::Regex;
use reqwest::Client;

#[derive(Debug)]
struct ProviderRequest {
    body: serde_json::Value,
    endpoint_url: String,
    response_format: ResponseFormat,
    provider: String,
    is_gemini_openai_compat: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResponseFormat {
    OpenAiCompatible,
    Gemini,
    OllamaNative,
}

fn build_gemini_generate_content_endpoint(endpoint: &str, model: &str, api_key: &str) -> String {
    let base = endpoint.trim_end_matches('/');
    if base.contains(":generateContent") {
        if base.contains("key=") {
            return base.to_string();
        }
        let separator = if base.contains('?') { "&" } else { "?" };
        return format!("{}{}key={}", base, separator, api_key);
    }

    let model = model.trim_start_matches("models/");
    if base.contains("/models/") {
        return format!("{}:generateContent?key={}", base, api_key);
    }

    format!("{}/models/{}:generateContent?key={}", base, model, api_key)
}

fn build_provider_request(
    config: &LLMConfig,
    lines: &[TranslationLine],
    source_lang: &str,
    target_lang: &str,
) -> Result<ProviderRequest, String> {
    let system_prompt = build_translation_prompt(&config.system_prompt, source_lang, target_lang);
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
                        "text": format!("{}\n\nTranslate the following:\n{}", system_prompt, user_content)
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

pub async fn call_llm_api(
    config: &LLMConfig,
    lines: &[TranslationLine],
    source_lang: &str,
    target_lang: &str,
) -> Result<Vec<TranslatedLine>, String> {
    let client = Client::new();
    let provider_request = build_provider_request(config, lines, source_lang, target_lang)?;
    let mut request = client
        .post(&provider_request.endpoint_url)
        .json(&provider_request.body);

    if provider_request.is_gemini_openai_compat {
        request = request.header("Authorization", format!("Bearer {}", config.api_key));
    } else {
        match provider_request.provider.as_str() {
            "openai" | "openrouter" | "custom" | "minimax" | "nvidia" => {
                if !config.api_key.is_empty() {
                    request = request.header("Authorization", format!("Bearer {}", config.api_key));
                }
                if provider_request.provider == "openrouter" {
                    request = request.header("HTTP-Referer", "https://animesubs.app");
                }
            }
            _ => {}
        }
    }

    eprintln!(
        "Calling LLM API: {} with model {}",
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

pub fn parse_translation_response_content(content: &str) -> Result<Vec<TranslatedLine>, String> {
    let thinking_regex = Regex::new(r"(?is)<(?:thinking|think)>.*?</(?:thinking|think)>").unwrap();
    let content_without_thinking = thinking_regex.replace_all(content, "").to_string();
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

fn text_from_content_value(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value.as_str() {
        return Some(text.to_string());
    }

    let parts = value.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| {
            part.get("text")
                .or_else(|| part.get("content"))
                .and_then(|value| value.as_str())
        })
        .collect::<Vec<_>>()
        .join("");

    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn extract_response_content(
    response_json: &serde_json::Value,
    response_format: ResponseFormat,
) -> Result<String, String> {
    match response_format {
        ResponseFormat::OpenAiCompatible => {
            let content = &response_json["choices"][0]["message"]["content"];
            text_from_content_value(content)
                .ok_or_else(|| "Missing content in OpenAI response".to_string())
        }
        ResponseFormat::Gemini => {
            let parts = response_json["candidates"][0]["content"]["parts"]
                .as_array()
                .ok_or_else(|| "Missing content in Gemini response".to_string())?;
            let text = parts
                .iter()
                .filter(|part| part.get("thought").and_then(|value| value.as_bool()) != Some(true))
                .filter_map(|part| part.get("text").and_then(|value| value.as_str()))
                .collect::<Vec<_>>()
                .join("");

            if text.is_empty() {
                Err("Missing content in Gemini response".to_string())
            } else {
                Ok(text)
            }
        }
        ResponseFormat::OllamaNative => response_json["message"]["content"]
            .as_str()
            .map(|content| content.to_string())
            .ok_or_else(|| "Missing content in Ollama response".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    #[derive(Debug)]
    struct CapturedRequest {
        method: String,
        path: String,
        headers: HashMap<String, String>,
        body: serde_json::Value,
    }

    impl CapturedRequest {
        fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .get(&name.to_ascii_lowercase())
                .map(|value| value.as_str())
        }
    }

    fn sample_lines() -> Vec<TranslationLine> {
        vec![TranslationLine {
            id: 7,
            text: "こんにちは".to_string(),
        }]
    }

    fn config(provider: &str, endpoint: String) -> LLMConfig {
        LLMConfig {
            provider: provider.to_string(),
            api_key: "test-key".to_string(),
            endpoint,
            model: "test-model".to_string(),
            system_prompt: "natural".to_string(),
        }
    }

    fn openai_response(content: &str) -> String {
        serde_json::json!({
            "choices": [{
                "message": {
                    "content": content,
                    "reasoning_content": "internal reasoning should be ignored"
                }
            }]
        })
        .to_string()
    }

    fn translation_content(text: &str) -> String {
        serde_json::json!({
            "translations": [{"id": 7, "text": text}]
        })
        .to_string()
    }

    fn find_header_end(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|window| window == b"\r\n\r\n")
    }

    fn content_length(head: &str) -> usize {
        head.lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                if name.eq_ignore_ascii_case("content-length") {
                    value.trim().parse().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0)
    }

    fn status_text(status: u16) -> &'static str {
        match status {
            200 => "OK",
            400 => "Bad Request",
            500 => "Internal Server Error",
            _ => "Test Status",
        }
    }

    async fn start_test_server(
        status: u16,
        response_body: String,
    ) -> (String, oneshot::Receiver<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, receiver) = oneshot::channel();

        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            let mut chunk = [0u8; 1024];

            loop {
                let read = socket.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..read]);

                if let Some(header_end) = find_header_end(&buffer) {
                    let head = String::from_utf8_lossy(&buffer[..header_end]);
                    let expected_len = header_end + 4 + content_length(&head);
                    if buffer.len() >= expected_len {
                        break;
                    }
                }
            }

            let header_end = find_header_end(&buffer).unwrap();
            let head = String::from_utf8_lossy(&buffer[..header_end]);
            let body_start = header_end + 4;
            let body_text = String::from_utf8_lossy(&buffer[body_start..]).to_string();
            let mut lines = head.lines();
            let request_line = lines.next().unwrap();
            let mut request_parts = request_line.split_whitespace();
            let method = request_parts.next().unwrap().to_string();
            let path = request_parts.next().unwrap().to_string();
            let headers = lines
                .filter_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    Some((name.to_ascii_lowercase(), value.trim().to_string()))
                })
                .collect();

            sender
                .send(CapturedRequest {
                    method,
                    path,
                    headers,
                    body: serde_json::from_str(&body_text).unwrap(),
                })
                .unwrap();

            let response = format!(
                "HTTP/1.1 {} {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                status,
                status_text(status),
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        (format!("http://{}", address), receiver)
    }

    fn assert_translated(translations: Vec<TranslatedLine>, expected_text: &str) {
        assert_eq!(translations.len(), 1);
        assert_eq!(translations[0].id, 7);
        assert_eq!(translations[0].text, expected_text);
    }

    #[test]
    fn parses_response_with_thinking_and_markdown_noise() {
        let content = r#"<thinking>
I should return JSON only.
</thinking>

```json
{"translations":[{"id":0,"text":"Olá"},{"id":1,"text":"Mundo"}]}
```
extra text"#;

        let translations = parse_translation_response_content(content).unwrap();

        assert_eq!(translations.len(), 2);
        assert_eq!(translations[0].id, 0);
        assert_eq!(translations[0].text, "Olá");
        assert_eq!(translations[1].text, "Mundo");
    }

    #[test]
    fn parses_response_with_think_tags_from_reasoning_models() {
        let content = r#"<think>
I should reason internally.
</think>

Before JSON
```json
{"translations":[{"id":7,"text":"Olá"}]}
```
after JSON"#;

        let translations = parse_translation_response_content(content).unwrap();

        assert_translated(translations, "Olá");
    }

    #[test]
    fn reports_invalid_json() {
        let error = parse_translation_response_content("not json").unwrap_err();

        assert!(error.contains("Failed to parse translation JSON"));
    }

    #[test]
    fn builds_gemini_native_generate_content_endpoints() {
        assert_eq!(
            build_gemini_generate_content_endpoint("http://api.test/v1beta", "gemini-test", "key"),
            "http://api.test/v1beta/models/gemini-test:generateContent?key=key"
        );
        assert_eq!(
            build_gemini_generate_content_endpoint(
                "http://api.test/v1beta/models/gemini-test",
                "ignored",
                "key"
            ),
            "http://api.test/v1beta/models/gemini-test:generateContent?key=key"
        );
        assert_eq!(
            build_gemini_generate_content_endpoint(
                "http://api.test/v1beta/models/gemini-test:generateContent?key=existing",
                "ignored",
                "key"
            ),
            "http://api.test/v1beta/models/gemini-test:generateContent?key=existing"
        );
    }

    #[tokio::test]
    async fn openai_compatible_providers_call_chat_completions_and_parse_plain_json() {
        struct Case {
            provider: &'static str,
            endpoint_suffix: &'static str,
            expected_path: &'static str,
            expects_auth: bool,
            expects_referer: bool,
        }

        let cases = [
            Case {
                provider: "openai",
                endpoint_suffix: "",
                expected_path: "/chat/completions",
                expects_auth: true,
                expects_referer: false,
            },
            Case {
                provider: "openrouter",
                endpoint_suffix: "",
                expected_path: "/chat/completions",
                expects_auth: true,
                expects_referer: true,
            },
            Case {
                provider: "custom",
                endpoint_suffix: "",
                expected_path: "/chat/completions",
                expects_auth: true,
                expects_referer: false,
            },
            Case {
                provider: "minimax",
                endpoint_suffix: "",
                expected_path: "/chat/completions",
                expects_auth: true,
                expects_referer: false,
            },
            Case {
                provider: "nvidia",
                endpoint_suffix: "",
                expected_path: "/chat/completions",
                expects_auth: true,
                expects_referer: false,
            },
            Case {
                provider: "lmstudio",
                endpoint_suffix: "/v1",
                expected_path: "/v1/chat/completions",
                expects_auth: false,
                expects_referer: false,
            },
            Case {
                provider: "llamacpp",
                endpoint_suffix: "/v1",
                expected_path: "/v1/chat/completions",
                expects_auth: false,
                expects_referer: false,
            },
            Case {
                provider: "gemini",
                endpoint_suffix: "/openai",
                expected_path: "/openai/chat/completions",
                expects_auth: true,
                expects_referer: false,
            },
            Case {
                provider: "ollama",
                endpoint_suffix: "/v1",
                expected_path: "/v1/chat/completions",
                expects_auth: false,
                expects_referer: false,
            },
        ];

        for case in cases {
            let (base_url, request) =
                start_test_server(200, openai_response(&translation_content("Olá"))).await;
            let endpoint = format!("{}{}", base_url, case.endpoint_suffix);
            let translations = call_llm_api(
                &config(case.provider, endpoint),
                &sample_lines(),
                "ja",
                "pt",
            )
            .await
            .unwrap();
            let request = request.await.unwrap();

            assert_translated(translations, "Olá");
            assert_eq!(request.method, "POST");
            assert_eq!(
                request.path, case.expected_path,
                "provider: {}",
                case.provider
            );
            assert_eq!(request.body["model"], "test-model");
            assert_eq!(request.body["temperature"], 0.3);
            assert_eq!(request.body["response_format"]["type"], "json_object");
            assert!(request.body["messages"][0]["content"]
                .as_str()
                .unwrap()
                .contains("Translate from ja to pt"));
            assert!(request.body["messages"][1]["content"]
                .as_str()
                .unwrap()
                .contains("こんにちは"));

            if case.expects_auth {
                assert_eq!(request.header("authorization"), Some("Bearer test-key"));
            } else {
                assert_eq!(request.header("authorization"), None);
            }

            if case.expects_referer {
                assert_eq!(
                    request.header("http-referer"),
                    Some("https://animesubs.app")
                );
            } else {
                assert_eq!(request.header("http-referer"), None);
            }
        }
    }

    #[tokio::test]
    async fn openai_compatible_reasoning_models_ignore_reasoning_and_parse_content() {
        for provider in ["openrouter", "nvidia", "minimax"] {
            let reasoning_content = format!(
                "<think>hidden chain of thought</think>\n```json\n{}\n```",
                translation_content("Reasoning OK")
            );
            let (base_url, request) =
                start_test_server(200, openai_response(&reasoning_content)).await;

            let translations =
                call_llm_api(&config(provider, base_url), &sample_lines(), "ja", "en")
                    .await
                    .unwrap();
            let request = request.await.unwrap();

            assert_translated(translations, "Reasoning OK");
            assert_eq!(request.path, "/chat/completions");
            assert_eq!(request.header("authorization"), Some("Bearer test-key"));
        }
    }

    #[tokio::test]
    async fn gemini_native_calls_generate_content_and_skips_thought_parts() {
        let response = serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"thought": true, "text": "internal reasoning"},
                        {"text": format!("```json\n{}\n```", translation_content("Gemini OK"))}
                    ]
                }
            }]
        })
        .to_string();
        let (base_url, request) = start_test_server(200, response).await;
        let endpoint = format!("{}/v1beta", base_url);

        let translations = call_llm_api(&config("gemini", endpoint), &sample_lines(), "ja", "en")
            .await
            .unwrap();
        let request = request.await.unwrap();

        assert_translated(translations, "Gemini OK");
        assert_eq!(
            request.path,
            "/v1beta/models/test-model:generateContent?key=test-key"
        );
        assert_eq!(request.header("authorization"), None);
        assert_eq!(request.body["generationConfig"]["temperature"], 0.3);
        assert_eq!(
            request.body["generationConfig"]["responseMimeType"],
            "application/json"
        );
        assert!(request.body["contents"][0]["parts"][0]["text"]
            .as_str()
            .unwrap()
            .contains("こんにちは"));
    }

    #[tokio::test]
    async fn ollama_native_calls_api_chat_and_parses_thinking_response() {
        let response = serde_json::json!({
            "message": {
                "role": "assistant",
                "thinking": "internal reasoning",
                "content": format!("<thinking>hidden</thinking>{}", translation_content("Ollama OK"))
            },
            "done": true
        })
        .to_string();
        let (base_url, request) = start_test_server(200, response).await;

        let translations = call_llm_api(&config("ollama", base_url), &sample_lines(), "ja", "en")
            .await
            .unwrap();
        let request = request.await.unwrap();

        assert_translated(translations, "Ollama OK");
        assert_eq!(request.path, "/api/chat");
        assert_eq!(request.header("authorization"), None);
        assert_eq!(request.body["model"], "test-model");
        assert_eq!(request.body["stream"], false);
        assert_eq!(request.body["format"], "json");
        assert_eq!(request.body["options"]["temperature"], 0.3);
    }

    #[tokio::test]
    async fn reports_http_errors_with_provider_body() {
        let (base_url, _request) =
            start_test_server(400, serde_json::json!({"error":"bad request"}).to_string()).await;

        let error = call_llm_api(&config("openai", base_url), &sample_lines(), "ja", "en")
            .await
            .unwrap_err();

        assert!(error.contains("LLM API error (400 Bad Request)"));
        assert!(error.contains("bad request"));
    }

    #[test]
    fn reports_missing_content_for_each_response_format() {
        assert_eq!(
            extract_response_content(
                &serde_json::json!({"choices":[{"message":{}}]}),
                ResponseFormat::OpenAiCompatible
            )
            .unwrap_err(),
            "Missing content in OpenAI response"
        );
        assert_eq!(
            extract_response_content(
                &serde_json::json!({"candidates":[{"content":{"parts":[]}}]}),
                ResponseFormat::Gemini
            )
            .unwrap_err(),
            "Missing content in Gemini response"
        );
        assert_eq!(
            extract_response_content(
                &serde_json::json!({"message":{}}),
                ResponseFormat::OllamaNative
            )
            .unwrap_err(),
            "Missing content in Ollama response"
        );
    }

    #[tokio::test]
    async fn rejects_unsupported_provider_before_http_call() {
        let error = call_llm_api(
            &config("unknown", "http://127.0.0.1:9".to_string()),
            &sample_lines(),
            "ja",
            "en",
        )
        .await
        .unwrap_err();

        assert_eq!(error, "Unsupported provider: unknown");
    }
}
