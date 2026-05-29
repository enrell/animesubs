pub mod deepl;
pub mod ollama;
pub mod openai;

use async_trait::async_trait;

use crate::error::Result;

/// A translation backend.
///
/// Implement this trait to provide custom translation providers.
/// Built-in implementations:
/// - [`ollama::OllamaTranslator`] — calls the Ollama `/api/generate` endpoint
/// - [`openai::OpenAiTranslator`] — calls the OpenAI `/v1/chat/completions` endpoint
/// - [`deepl::DeepLTranslator`] — calls the DeepL `/v2/translate` endpoint
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
}

/// A translation request sent to a [`Translator`].
#[derive(Debug, Clone)]
pub struct TranslationRequest<'a> {
    /// Numbered subtitle dialogue text in `<N> text` format.
    pub source_text: &'a str,
    /// Target language code (e.g. `"pt-BR"`, `"en"`, `"ja"`).
    pub target_language: &'a str,
}
