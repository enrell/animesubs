use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator, prompt::build_translation_prompt};

#[derive(Debug, Clone)]
pub struct OllamaTranslator {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaTranslator {
    pub fn new(model: impl Into<String>) -> Self {
        Self::with_base_url("http://localhost:11434", model)
    }

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
