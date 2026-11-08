use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{
    TranslationLimits, TranslationRequest, Translator, provider_http_error,
    provider_transport_error,
};

/// Translator backend that calls the [OpenRouter](https://openrouter.ai) `/api/v1/chat/completions` endpoint.
///
/// OpenRouter provides an OpenAI-compatible API to 400+ models, including
/// free models (no credit card required). Model slugs use the format
/// `provider/model-name` (e.g. `meta-llama/llama-3.3-70b-instruct:free`).
///
/// # Example
///
/// ```no_run
/// # async fn example() -> animesubs_engine::Result<()> {
/// use animesubs_engine::OpenRouterTranslator;
///
/// let translator = OpenRouterTranslator::new("your-api-key", "meta-llama/llama-3.3-70b-instruct:free")?;
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OpenRouterTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenRouterTranslator {
    /// Create a new translator targeting the default OpenRouter API (`https://openrouter.ai/api`).
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://openrouter.ai/api", api_key, model)
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
impl Translator for OpenRouterTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let messages = build_messages(&request);

        let response = self
            .client
            .post(format!("{}/v1/chat/completions", self.base_url))
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header(
                "HTTP-Referer",
                "https://github.com/enrell/animesubs",
            )
            .header("X-Title", "animesubs")
            .json(&ChatCompletionRequest {
                model: &self.model,
                messages,
                stream: false,
            })
            .send()
            .await
            .map_err(|error| provider_transport_error("openrouter", error))?;

        if !response.status().is_success() {
            return Err(provider_http_error("openrouter", response).await);
        }

        let body = response.json::<ChatCompletionResponse>().await?;
        let choice = body
            .choices
            .first()
            .ok_or_else(|| SubtitleToolkitError::Translation {
                provider: "openrouter",
                message: "response contained no choices".into(),
            })?;
        match choice.finish_reason.as_deref() {
            Some("stop") => {}
            Some(reason) => {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: format!("openrouter response was incomplete (finish_reason={reason})"),
                });
            }
            None => {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: "openrouter response omitted finish_reason".into(),
                });
            }
        }
        let content = choice.message.content.trim().to_string();

        Ok(content)
    }

    fn identifier(&self) -> String {
        format!("openrouter:{}:{}", self.base_url, self.model)
    }

    fn limits(&self) -> TranslationLimits {
        TranslationLimits {
            reject_unchanged_output: true,
            ..TranslationLimits::default()
        }
    }
}

fn build_messages(request: &TranslationRequest<'_>) -> Vec<ChatMessage> {
    vec![
        ChatMessage {
            role: "system",
            content: "You are a subtitle translator. You translate subtitle dialogue while \
                      preserving numbered tags exactly. Return only the translated lines. \
                      Preserve tokens such as [[PSY_TAG_0]] exactly and in order. \
                      Do not add explanations, markdown, notes, or code fences. \
                      Do not add curly-brace commands or backslash formatting."
                .to_string(),
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
    #[serde(default)]
    finish_reason: Option<String>,
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
                    "message": { "content": "<1> Olá\n<2> mundo" },
                    "finish_reason": "stop"
                }]
            })))
            .mount(&server)
            .await;

        let translator = OpenRouterTranslator::with_base_url(
            server.uri(),
            "sk-test",
            "meta-llama/llama-3.3-70b-instruct:free",
        )
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
    async fn sends_bearer_auth_and_attribution_headers() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(header("Authorization", "Bearer sk-my-key"))
            .and(header(
                "HTTP-Referer",
                "https://github.com/enrell/animesubs",
            ))
            .and(header("X-Title", "animesubs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": { "content": "<1> ok" },
                    "finish_reason": "stop"
                }]
            })))
            .mount(&server)
            .await;

        let translator =
            OpenRouterTranslator::with_base_url(server.uri(), "sk-my-key", "some/model").unwrap();
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
            OpenRouterTranslator::with_base_url(server.uri(), "sk-bad", "some/model").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "en",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("openrouter"));
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
            OpenRouterTranslator::with_base_url(server.uri(), "sk-test", "some/model").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "en",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no choices"));
    }

    #[tokio::test]
    async fn rejects_token_limited_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": { "content": "<1> partial" },
                    "finish_reason": "length"
                }]
            })))
            .mount(&server)
            .await;
        let translator =
            OpenRouterTranslator::with_base_url(server.uri(), "sk-test", "model").unwrap();
        let error = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap_err();
        assert!(error.to_string().contains("finish_reason=length"));
    }
}
