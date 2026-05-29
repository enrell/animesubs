use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator};

/// Translator backend that calls the [OpenAI](https://platform.openai.com) `/v1/chat/completions` endpoint.
///
/// Compatible with any OpenAI-compatible API (OpenAI, Azure OpenAI, local proxies, etc.)
/// by setting a custom base URL via [`with_base_url`].
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::OpenAiTranslator;
///
/// let translator = OpenAiTranslator::new("sk-...", "gpt-4o-mini")?;
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OpenAiTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiTranslator {
    /// Create a new translator targeting the default OpenAI API (`https://api.openai.com`).
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://api.openai.com", api_key, model)
    }

    /// Create a new translator with a custom base URL, API key, and model.
    ///
    /// The base URL should not include `/v1/chat/completions` — just the origin
    /// (e.g. `https://api.openai.com` or `http://localhost:8080`).
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
impl Translator for OpenAiTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let messages = build_messages(&request);

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&ChatCompletionRequest {
                model: &self.model,
                messages,
                stream: false,
            })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SubtitleToolkitError::Translation {
                provider: "openai",
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|_| "request failed".into()),
            });
        }

        let body = response.json::<ChatCompletionResponse>().await?;
        let content = body
            .choices
            .first()
            .ok_or_else(|| SubtitleToolkitError::Translation {
                provider: "openai",
                message: "response contained no choices".into(),
            })?
            .message
            .content
            .trim()
            .to_string();

        Ok(content)
    }
}

fn build_messages(request: &TranslationRequest<'_>) -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "system",
            content: "You are a subtitle translator. You translate subtitle dialogue while \
                      preserving numbered tags exactly. Return only the translated lines. \
                      Do not add explanations, markdown, notes, or code fences. \
                      Do not add curly-brace commands or backslash formatting."
                .into(),
        },
        ChatMessage {
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
        },
    ]
}

#[derive(Debug, Serialize)]
struct ChatMessage {
    role: &'static str,
    content: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct ChatChoiceMessage {
    content: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn translates_numbered_text() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": { "content": "<1> Olá\n<2> mundo" }
                }]
            })))
            .mount(&server)
            .await;

        let translator =
            OpenAiTranslator::with_base_url(server.uri(), "sk-test", "gpt-4o-mini").unwrap();
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
    async fn sends_bearer_auth_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(header("Authorization", "Bearer sk-my-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{ "message": { "content": "<1> ok" } }]
            })))
            .mount(&server)
            .await;

        let translator =
            OpenAiTranslator::with_base_url(server.uri(), "sk-my-key", "gpt-4o-mini").unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "en",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn error_on_non_200() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(401).set_body_string(r#"{"error": "invalid api key"}"#),
            )
            .mount(&server)
            .await;

        let translator =
            OpenAiTranslator::with_base_url(server.uri(), "sk-bad", "gpt-4o-mini").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "en",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("openai"));
    }

    #[tokio::test]
    async fn error_on_empty_choices() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": []
            })))
            .mount(&server)
            .await;

        let translator =
            OpenAiTranslator::with_base_url(server.uri(), "sk-test", "gpt-4o-mini").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "en",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no choices"));
    }
}
