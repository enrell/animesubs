pub mod ollama;
pub mod prompt;

use async_trait::async_trait;

use crate::error::Result;

/// A translation backend.
///
/// Implement this trait to provide custom translation providers.
/// The built-in [`ollama::OllamaTranslator`] calls the Ollama `/api/generate` endpoint.
#[async_trait]
pub trait Translator {
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
