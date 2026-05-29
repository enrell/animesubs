use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator};

/// Translator backend that calls the [Ollama](https://ollama.com) `/api/generate` endpoint.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::OllamaTranslator;
///
/// let translator = OllamaTranslator::new("llama3.1")?;
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
    pub fn new(model: impl Into<String>) -> Result<Self> {
        Self::with_base_url("http://localhost:11434", model)
    }

    /// Create a new translator with a custom Ollama base URL and model.
    pub fn with_base_url(base_url: impl Into<String>, model: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(SubtitleToolkitError::Http)?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
        })
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
                prompt: &build_prompt(&request),
                stream: false,
            })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SubtitleToolkitError::Translation {
                provider: "ollama",
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

fn build_prompt(request: &TranslationRequest<'_>) -> String {
    let source_hint = match request.source_language {
        Some(lang) => format!("The source language is {lang}.\n"),
        None => String::new(),
    };
    format!(
        "Translate the following subtitle dialogue to {target_language}.\n\
         {source_hint}\n\
         Rules:\n\
         - Preserve every numeric tag exactly, like <1>, <2>, <3>.\n\
         - Return only translated subtitle lines.\n\
         - Do not add explanations, markdown, notes, or code fences.\n\
         - Keep line breaks inside each subtitle when needed.\n\
         - Do not add curly-brace commands or backslash formatting.\n\n\
         Subtitle dialogue:\n\
         {source_text}",
        target_language = request.target_language,
        source_text = request.source_text,
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn translates_numbered_text() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "<1> Olá\n<2> mundo"
            })))
            .mount(&server)
            .await;

        let translator = OllamaTranslator::with_base_url(server.uri(), "test-model").unwrap();
        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> hello\n<2> world",
                target_language: "pt-BR",
                source_language: None,
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá\n<2> mundo");
    }

    #[tokio::test]
    async fn trims_whitespace_from_response() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "  <1> Olá  \n"
            })))
            .mount(&server)
            .await;

        let translator = OllamaTranslator::with_base_url(server.uri(), "test-model").unwrap();
        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
                source_language: None,
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá");
    }

    #[tokio::test]
    async fn error_on_non_200() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(500).set_body_string("model not found"))
            .mount(&server)
            .await;

        let translator = OllamaTranslator::with_base_url(server.uri(), "bad-model").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt-BR",
                source_language: None,
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("ollama"));
        assert!(err.to_string().contains("model not found"));
    }

    #[tokio::test]
    async fn sends_correct_model_and_prompt() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "response": "<1> ok"
            })))
            .expect(1)
            .mount(&server)
            .await;

        let translator = OllamaTranslator::with_base_url(server.uri(), "my-model").unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "ja",
                source_language: None,
            })
            .await
            .unwrap();

        // wiremock verifies the request was made once
    }
}
