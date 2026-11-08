//! First-class OpenCode Zen and OpenCode Go Chat Completions providers.
//!
//! These services expose selected models through an OpenAI-compatible
//! `/v1/chat/completions` endpoint. Models using `/responses` or `/messages`
//! require a provider adapter for those protocols instead.

use async_trait::async_trait;

use crate::error::Result;

use super::openai::OpenAiCompatibleTranslator;
use super::{TranslationLimits, TranslationRequest, Translator};

const OPENCODE_ZEN_URL: &str = "https://opencode.ai/zen";
const OPENCODE_GO_URL: &str = "https://opencode.ai/zen/go";

/// Translator for OpenCode Zen's OpenAI-compatible Chat Completions endpoint.
#[derive(Debug, Clone)]
pub struct OpenCodeZenTranslator {
    inner: OpenAiCompatibleTranslator,
}

impl OpenCodeZenTranslator {
    /// Create a translator using the current OpenCode Zen endpoint.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_base_url(OPENCODE_ZEN_URL, api_key, model)
    }

    /// Create a Zen translator with a custom compatible base URL.
    pub fn with_base_url(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: OpenAiCompatibleTranslator::with_base_url(
                base_url,
                api_key,
                model,
                "opencode-zen",
            )?,
        })
    }
}

#[async_trait]
impl Translator for OpenCodeZenTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        self.inner.translate(request).await
    }

    fn identifier(&self) -> String {
        self.inner.identifier()
    }

    fn limits(&self) -> TranslationLimits {
        self.inner.limits()
    }
}

/// Translator for OpenCode Go's OpenAI-compatible Chat Completions endpoint.
#[derive(Debug, Clone)]
pub struct OpenCodeGoTranslator {
    inner: OpenAiCompatibleTranslator,
}

impl OpenCodeGoTranslator {
    /// Create a translator using the current OpenCode Go endpoint.
    pub fn new(api_key: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        Self::with_base_url(OPENCODE_GO_URL, api_key, model)
    }

    /// Create a Go translator with a custom compatible base URL.
    pub fn with_base_url(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            inner: OpenAiCompatibleTranslator::with_base_url(
                base_url,
                api_key,
                model,
                "opencode-go",
            )?,
        })
    }
}

#[async_trait]
impl Translator for OpenCodeGoTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        self.inner.translate(request).await
    }

    fn identifier(&self) -> String {
        self.inner.identifier()
    }

    fn limits(&self) -> TranslationLimits {
        self.inner.limits()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn mount_chat_response(server: &MockServer) {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "choices": [{
                    "message": {"content": "<1> Olá"},
                    "finish_reason": "stop"
                }]
            })))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn zen_uses_the_chat_completions_contract() {
        let server = MockServer::start().await;
        mount_chat_response(&server).await;
        let translator =
            OpenCodeZenTranslator::with_base_url(server.uri(), "key", "model").unwrap();

        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> Hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá");
        assert!(translator.identifier().starts_with("opencode-zen:"));
    }

    #[tokio::test]
    async fn go_uses_the_chat_completions_contract() {
        let server = MockServer::start().await;
        mount_chat_response(&server).await;
        let translator = OpenCodeGoTranslator::with_base_url(server.uri(), "key", "model").unwrap();

        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> Hello",
                target_language: "pt-BR",
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá");
        assert!(translator.identifier().starts_with("opencode-go:"));
    }
}
