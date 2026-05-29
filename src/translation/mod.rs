pub mod ollama;
pub mod prompt;

use async_trait::async_trait;

use crate::error::Result;

#[async_trait]
pub trait Translator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String>;
}

#[derive(Debug, Clone)]
pub struct TranslationRequest<'a> {
    pub source_text: &'a str,
    pub target_language: &'a str,
}
