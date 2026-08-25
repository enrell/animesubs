use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{
    TranslationLimits, TranslationRequest, Translator, provider_http_error,
    provider_transport_error,
};

/// Translator backend that calls the [Google Gemini](https://ai.google.dev/gemini-api/docs) `generateContent` endpoint.
///
/// Uses API key authentication via the `x-goog-api-key` header.
/// Free tier includes 1,500 requests/day on Flash models.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::GeminiTranslator;
///
/// let translator = GeminiTranslator::new("your-api-key", "gemini-2.5-flash-lite")?;
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GeminiTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl GeminiTranslator {
    /// Create a new translator targeting the default Gemini API
    /// (`https://generativelanguage.googleapis.com`) with the given model.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://generativelanguage.googleapis.com", api_key, model)
    }

    /// Create a new translator with a custom base URL, API key, and model.
    pub fn with_base_url(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(SubtitleToolkitError::Http)?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
        })
    }
}

#[async_trait::async_trait]
impl Translator for GeminiTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let url = format!(
            "{}/v1beta/models/{}:generateContent",
            self.base_url, self.model
        );

        let response = self
            .client
            .post(&url)
            .header("x-goog-api-key", &self.api_key)
            .header("Content-Type", "application/json")
            .json(&GenerateContentRequest {
                system_instruction: SystemInstruction {
                    parts: vec![Part {
                        text: "You are a subtitle translator. \
                               Translate the user text to the requested language. \
                               Preserve every numeric tag exactly, like <1>, <2>, <3>. \
                               Preserve tokens such as [[PSY_TAG_0]] exactly and in order. \
                               Return only translated subtitle lines. \
                               Do not add explanations, markdown, notes, or code fences. \
                               Keep line breaks inside each subtitle when needed. \
                               Do not add curly-brace commands or backslash formatting."
                            .to_string(),
                    }],
                },
                contents: vec![Content {
                    parts: vec![Part {
                        text: format!(
                            "Target language: {lang}\n\n{text}",
                            lang = request.target_language,
                            text = request.source_text,
                        ),
                    }],
                }],
                generation_config: GenerationConfig { temperature: 0.2 },
            })
            .send()
            .await
            .map_err(|error| provider_transport_error("gemini", error))?;

        if !response.status().is_success() {
            return Err(provider_http_error("gemini", response).await);
        }

        let body = response.json::<GenerateContentResponse>().await?;
        let candidate = body.candidates.into_iter().next().ok_or_else(|| {
            SubtitleToolkitError::Translation {
                provider: "gemini",
                message: "response contained no candidates".into(),
            }
        })?;
        match candidate.finish_reason.as_deref() {
            Some("STOP") => {}
            Some(reason) => {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: format!("gemini response was incomplete (finishReason={reason})"),
                });
            }
            None => {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: "gemini response omitted finishReason".into(),
                });
            }
        }
        let text = candidate
            .content
            .parts
            .into_iter()
            .filter_map(|part| part.text)
            .collect::<String>();
        if text.trim().is_empty() {
            return Err(SubtitleToolkitError::Translation {
                provider: "gemini",
                message: "response candidate contained no text parts".into(),
            });
        }

        Ok(text.trim().to_string())
    }

    fn identifier(&self) -> String {
        format!("gemini:{}:{}", self.base_url, self.model)
    }

    fn limits(&self) -> TranslationLimits {
        TranslationLimits {
            reject_unchanged_output: true,
            ..TranslationLimits::default()
        }
    }
}

#[derive(Debug, Serialize)]
struct GenerateContentRequest {
    system_instruction: SystemInstruction,
    contents: Vec<Content>,
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct SystemInstruction {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct GenerateContentResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: CandidateContent,
    #[serde(default, rename = "finishReason")]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CandidateContent {
    parts: Vec<CandidatePart>,
}

#[derive(Debug, Deserialize)]
struct CandidatePart {
    #[serde(default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path_regex};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn gemini_response(text: &str) -> serde_json::Value {
        serde_json::json!({
            "candidates": [{
                "content": {
                    "parts": [{ "text": text }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        })
    }

    #[tokio::test]
    async fn translates_numbered_text() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path_regex(r"/v1beta/models/.*:generateContent"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(gemini_response("<1> Olá\n<2> mundo")),
            )
            .mount(&server)
            .await;

        let translator =
            GeminiTranslator::with_base_url(server.uri(), "test-key", "gemini-2.0-flash").unwrap();
        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> hello\n<2> world",
                target_language: "pt-BR",
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá\n<2> mundo");
    }

    #[tokio::test]
    async fn sends_api_key_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(wiremock::matchers::header(
                "x-goog-api-key",
                "my-secret-key",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(gemini_response("<1> ok")))
            .mount(&server)
            .await;

        let translator =
            GeminiTranslator::with_base_url(server.uri(), "my-secret-key", "gemini-2.0-flash")
                .unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "de",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn trims_whitespace_from_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(gemini_response("  <1> Olá  \n")),
            )
            .mount(&server)
            .await;

        let translator =
            GeminiTranslator::with_base_url(server.uri(), "test-key", "gemini-2.0-flash").unwrap();
        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá");
    }

    #[tokio::test]
    async fn error_on_non_200() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(429)
                    .set_body_string(r#"{"error": {"message": "Quota exceeded"}}"#),
            )
            .mount(&server)
            .await;

        let translator =
            GeminiTranslator::with_base_url(server.uri(), "bad-key", "gemini-2.0-flash").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("gemini"));
        assert!(err.to_string().contains("Quota exceeded"));
    }

    #[tokio::test]
    async fn error_on_empty_candidates() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({ "candidates": [] })),
            )
            .mount(&server)
            .await;

        let translator =
            GeminiTranslator::with_base_url(server.uri(), "test-key", "gemini-2.0-flash").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no candidates"));
    }

    #[tokio::test]
    async fn error_on_empty_parts() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "candidates": [{
                    "content": { "parts": [], "role": "model" },
                    "finishReason": "STOP"
                }]
            })))
            .mount(&server)
            .await;

        let translator =
            GeminiTranslator::with_base_url(server.uri(), "test-key", "gemini-2.0-flash").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no text parts"));
    }

    #[tokio::test]
    async fn rejects_max_tokens_finish_reason() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "candidates": [{
                    "content": { "parts": [{ "text": "<1> partial" }] },
                    "finishReason": "MAX_TOKENS"
                }]
            })))
            .mount(&server)
            .await;
        let translator =
            GeminiTranslator::with_base_url(server.uri(), "test-key", "gemini-test").unwrap();
        let error = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("finishReason=MAX_TOKENS"));
    }
}
