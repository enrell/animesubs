/// Anthropic Messages API provider (`/v1/messages`).
pub mod anthropic;
/// DeepL Translation API provider (`/v2/translate`).
pub mod deepl;
/// Google Gemini `generateContent` provider.
pub mod gemini;
/// Google Cloud Translation v2 provider (`/language/translate/v2`).
pub mod google;
/// Ollama local LLM provider (`/api/generate`).
pub mod ollama;
/// OpenAI Chat Completions provider (`/v1/chat/completions`).
pub mod openai;
/// OpenCode Zen and OpenCode Go Chat Completions providers.
pub mod opencode;
/// OpenRouter unified LLM provider (`/api/v1/chat/completions`).
pub mod openrouter;

use async_trait::async_trait;

use crate::error::{Result, SubtitleToolkitError};

/// Provider-specific request limits used by the pipeline chunker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslationLimits {
    /// Maximum numbered cues in one request.
    pub max_items: usize,
    /// Maximum UTF-8 bytes in the numbered request body.
    pub max_request_bytes: usize,
    /// Reject a response that is exactly identical to a non-trivial request.
    pub reject_unchanged_output: bool,
}

impl Default for TranslationLimits {
    fn default() -> Self {
        Self {
            max_items: 200,
            max_request_bytes: 48 * 1024,
            reject_unchanged_output: false,
        }
    }
}

/// A translation backend.
///
/// Implement this trait to provide custom translation providers.
/// Built-in implementations:
/// - [`anthropic::AnthropicTranslator`] — calls the Anthropic Messages API
/// - [`ollama::OllamaTranslator`] — calls the Ollama `/api/generate` endpoint
/// - [`openai::OpenAiTranslator`] — calls the OpenAI `/v1/chat/completions` endpoint
/// - [`deepl::DeepLTranslator`] — calls the DeepL `/v2/translate` endpoint
/// - [`google::GoogleTranslator`] — calls the Google Cloud Translation v2 endpoint
/// - [`gemini::GeminiTranslator`] — calls the Google Gemini `generateContent` endpoint
/// - [`openrouter::OpenRouterTranslator`] — calls the OpenRouter `/api/v1/chat/completions` endpoint
///
/// - [`opencode::OpenCodeZenTranslator`] calls OpenCode Zen's compatible endpoint
/// - [`opencode::OpenCodeGoTranslator`] calls OpenCode Go's compatible endpoint
///
/// Each provider owns its own prompt construction and HTTP client.
/// The pipeline calls [`Translator::translate`] with numbered subtitle text
/// in `<N> text` format and expects the same format back.
#[async_trait]
pub trait Translator: Send + Sync {
    /// Translate the source text and return the translated string.
    ///
    /// The returned string should preserve the `<N>` numbered line format
    /// from the input.
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String>;

    /// Stable, non-secret identifier used in resume manifests and diagnostics.
    fn identifier(&self) -> String {
        std::any::type_name::<Self>().to_string()
    }

    /// Request limits and validation behavior for this backend.
    fn limits(&self) -> TranslationLimits {
        TranslationLimits::default()
    }
}

/// A translation request sent to a [`Translator`].
#[derive(Debug, Clone)]
pub struct TranslationRequest<'a> {
    /// Numbered subtitle dialogue text in `<N> text` format.
    pub source_text: &'a str,
    /// Target language code (e.g. `"pt-BR"`, `"en"`, `"ja"`).
    pub target_language: &'a str,
}

/// Convert a non-success HTTP response into a structured provider error.
pub(crate) async fn provider_http_error(
    provider: &'static str,
    response: reqwest::Response,
) -> SubtitleToolkitError {
    const MAX_ERROR_BODY_CHARS: usize = 1_000;

    let status = response.status();
    let retry_after_seconds = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let retryable = matches!(status.as_u16(), 408 | 425 | 429) || status.is_server_error();
    let body = response
        .text()
        .await
        .unwrap_or_else(|_| "request failed and the response body could not be read".into());
    let mut message: String = body.chars().take(MAX_ERROR_BODY_CHARS).collect();
    if body.chars().count() > MAX_ERROR_BODY_CHARS {
        message.push_str("...");
    }

    SubtitleToolkitError::Provider {
        provider,
        status: Some(status.as_u16()),
        message,
        retryable,
        retry_after_seconds,
    }
}

/// Convert a transport failure without retaining a URL that may contain secrets.
pub(crate) fn provider_transport_error(
    provider: &'static str,
    error: reqwest::Error,
) -> SubtitleToolkitError {
    let retryable = error.is_timeout() || error.is_connect() || error.is_request();
    let message = if error.is_timeout() {
        "request timed out"
    } else if error.is_connect() {
        "connection failed"
    } else if error.is_request() {
        "request transport failed"
    } else {
        "request failed"
    };
    SubtitleToolkitError::Provider {
        provider,
        status: None,
        message: message.into(),
        retryable,
        retry_after_seconds: None,
    }
}
