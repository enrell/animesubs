use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator, prompt::build_translation_prompt};

/// Translator backend that calls the [Ollama](https://ollama.com) `/api/generate` endpoint.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::OllamaTranslator;
///
/// let translator = OllamaTranslator::new("llama3.1");
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct OllamaTranslator {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaTranslator {
    /// Create a new translator targeting `http://localhost:11434` with the given model.
    pub fn new(model: impl Into<String>) -> Self {
        Self::with_base_url("http://localhost:11434", model)
    }

    /// Create a new translator with a custom Ollama base URL and model.
    pub fn with_base_url(base_url: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
        }
    }
}

#[async_trait::async_trait]
impl Translator for OllamaTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/api/generate", self.base_url))
            .json(&OllamaGenerateRequest {
                model: &self.model,
                prompt: &build_translation_prompt(&request),
                stream: false,
            })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SubtitleToolkitError::Ollama {
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|_| "request failed".into()),
            });
        }

        let body = response.json::<OllamaGenerateResponse>().await?;
        Ok(body.response.trim().to_string())
    }
}

#[derive(Debug, Serialize)]
struct OllamaGenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaGenerateResponse {
    response: String,
}
