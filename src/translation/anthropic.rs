use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator};

/// Translator backend that calls the [Anthropic Messages API](https://docs.anthropic.com/en/api/messages).
///
/// Uses `x-api-key` header authentication. Default base URL is `https://api.anthropic.com`.
/// Supports custom endpoints for proxies or compatible APIs.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::AnthropicTranslator;
///
/// let translator = AnthropicTranslator::new("sk-ant-...", "claude-sonnet-4-6")?;
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct AnthropicTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl AnthropicTranslator {
    /// Create a new translator targeting the default Anthropic API (`https://api.anthropic.com`).
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://api.anthropic.com", api_key, model)
    }

    /// Create a new translator with a custom base URL, API key, and model.
    ///
    /// The base URL should not include `/v1/messages` — just the origin
    /// (e.g. `https://api.anthropic.com` or `http://localhost:8080`).
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
impl Translator for AnthropicTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&MessagesRequest {
                model: &self.model,
                max_tokens: 4096,
                system: "You are a subtitle translator. You translate subtitle dialogue while \
                     preserving numbered tags exactly. Return only the translated lines. \
                     Do not add explanations, markdown, notes, or code fences. \
                     Do not add curly-brace commands or backslash formatting."
                    .to_string(),
                messages: vec![Message {
                    role: "user",
                    content: format!(
                        "Translate the following subtitle dialogue to {target_language}.\n\n\
                         Preserve every numeric tag exactly, like <1>, <2>, <3>.\n\
                         Keep line breaks inside each subtitle when needed.\n\n\
                         Subtitle dialogue:\n\
                         {source_text}",
                        target_language = request.target_language,
                        source_text = request.source_text,
                    ),
                }],
                temperature: 0.2,
            })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SubtitleToolkitError::Translation {
                provider: "anthropic",
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|_| "request failed".into()),
            });
        }

        let body = response.json::<MessagesResponse>().await?;
        let text = body
            .content
            .into_iter()
            .find(|block| block.block_type == "text")
            .ok_or_else(|| SubtitleToolkitError::Translation {
                provider: "anthropic",
                message: "response contained no text content blocks".into(),
            })?
            .text;

        Ok(text.trim().to_string())
    }
}

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    system: String,
    messages: Vec<Message>,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct Message {
    role: &'static str,
    content: String,
}

#[derive(Debug, Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    block_type: String,
    #[serde(default)]
    text: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn anthropic_response(text: &str) -> serde_json::Value {
        serde_json::json!({
            "id": "msg_test",
            "type": "message",
            "role": "assistant",
            "content": [{ "type": "text", "text": text }],
            "model": "claude-sonnet-4-6",
            "stop_reason": "end_turn"
        })
    }

    #[tokio::test]
    async fn translates_numbered_text() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_response(
                "<1> Olá\n<2> mundo",
            )))
            .mount(&server)
            .await;

        let translator =
            AnthropicTranslator::with_base_url(server.uri(), "test-key", "claude-sonnet-4-6")
                .unwrap();
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
    async fn sends_auth_and_version_headers() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(header("x-api-key", "sk-ant-my-key"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_response("<1> ok")))
            .mount(&server)
            .await;

        let translator =
            AnthropicTranslator::with_base_url(server.uri(), "sk-ant-my-key", "claude-sonnet-4-6")
                .unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "en",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn trims_whitespace_from_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(anthropic_response(
                "  <1> Olá  \n",
            )))
            .mount(&server)
            .await;

        let translator =
            AnthropicTranslator::with_base_url(server.uri(), "test-key", "claude-sonnet-4-6")
                .unwrap();
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
                ResponseTemplate::new(401).set_body_string(r#"{"error": {"message": "invalid api key"}}"#),
            )
            .mount(&server)
            .await;

        let translator =
            AnthropicTranslator::with_base_url(server.uri(), "bad-key", "claude-sonnet-4-6")
                .unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "en",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("anthropic"));
        assert!(err.to_string().contains("invalid api key"));
    }

    #[tokio::test]
    async fn error_on_no_text_content_blocks() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "msg_test",
                "type": "message",
                "role": "assistant",
                "content": [{ "type": "tool_use", "id": "tool_1", "name": "test", "input": {} }],
                "model": "claude-sonnet-4-6",
                "stop_reason": "end_turn"
            })))
            .mount(&server)
            .await;

        let translator =
            AnthropicTranslator::with_base_url(server.uri(), "test-key", "claude-sonnet-4-6")
                .unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "en",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no text content blocks"));
    }
}
