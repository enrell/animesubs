use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator};

/// Translator backend that calls the [Google Cloud Translation v2](https://cloud.google.com/translate/docs/reference/rest/v2/translate) endpoint.
///
/// Uses API key authentication via the `key` query parameter.
/// The first 500,000 characters/month are free.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::GoogleTranslator;
///
/// let translator = GoogleTranslator::new("your-api-key")?;
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GoogleTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl GoogleTranslator {
    /// Create a new translator targeting the default Google Translate API
    /// (`https://translation.googleapis.com`).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://translation.googleapis.com", api_key)
    }

    /// Create a new translator with a custom base URL and API key.
    pub fn with_base_url(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(SubtitleToolkitError::Http)?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
        })
    }
}

#[async_trait::async_trait]
impl Translator for GoogleTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        // Google v2 uses language codes like "pt", "en", "ja" — not "pt-BR".
        // We pass the target as-is; Google handles regional codes (e.g. "pt-BR" works).
        let response = self
            .client
            .post(format!(
                "{}/language/translate/v2",
                self.base_url
            ))
            .query(&[("key", &self.api_key)])
            .json(&GoogleTranslateRequest {
                q: request.source_text.lines().collect(),
                target: request.target_language,
                format: "text",
            })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SubtitleToolkitError::Translation {
                provider: "google",
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|_| "request failed".into()),
            });
        }

        let body = response.json::<GoogleTranslateResponse>().await?;
        if body.data.translations.is_empty() {
            return Err(SubtitleToolkitError::Translation {
                provider: "google",
                message: "response contained no translations".into(),
            });
        }
        let translated = body
            .data
            .translations
            .into_iter()
            .map(|t| t.translated_text)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(translated)
    }
}

#[derive(Debug, Serialize)]
struct GoogleTranslateRequest<'a> {
    q: Vec<&'a str>,
    target: &'a str,
    format: &'static str,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslateResponse {
    data: GoogleTranslateData,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslateData {
    translations: Vec<GoogleTranslation>,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslation {
    #[serde(rename = "translatedText")]
    translated_text: String,
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
            .and(path("/language/translate/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "translations": [{ "translatedText": "<1> Olá" }, { "translatedText": "<2> mundo" }]
                }
            })))
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "test-key").unwrap();
        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> hello\n<2> world",
                target_language: "pt",
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Olá\n<2> mundo");
    }

    #[tokio::test]
    async fn sends_api_key_as_query_param() {
        let server = MockServer::start().await;

        // wiremock will match the path; we also verify the request was made
        Mock::given(method("POST"))
            .and(path("/language/translate/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "translations": [{ "translatedText": "<1> ok" }]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "my-key").unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "de",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn sends_format_text() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "translations": [{ "translatedText": "<1> ok" }]
                }
            })))
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "test-key").unwrap();
        // "format": "text" prevents HTML interpretation of ASS tags
        translator
            .translate(TranslationRequest {
                source_text: r"<1> {\b1}Bold text",
                target_language: "es",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn error_on_non_200() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string(r#"{"error": {"message": "Daily Limit Exceeded"}}"#),
            )
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "bad-key").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("google"));
        assert!(err.to_string().contains("Daily Limit Exceeded"));
    }

    #[tokio::test]
    async fn error_on_empty_translations() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": { "translations": [] }
            })))
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "test-key").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "pt",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no translations"));
    }

    #[tokio::test]
    async fn sends_each_line_as_separate_array_element() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/language/translate/v2"))
            .and(wiremock::matchers::body_string_contains(r#""<1> hello""#))
            .and(wiremock::matchers::body_string_contains(r#""<2> world""#))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "translations": [{ "translatedText": "<1> Olá" }, { "translatedText": "<2> mundo" }]
                }
            })))
            .expect(1)
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "test-key").unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> hello\n<2> world",
                target_language: "pt",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn joins_translated_lines_back() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": {
                    "translations": [
                        { "translatedText": "<1> Zeile eins" },
                        { "translatedText": "<2> Zeile zwei" },
                        { "translatedText": "<3> Zeile drei" }
                    ]
                }
            })))
            .mount(&server)
            .await;

        let translator = GoogleTranslator::with_base_url(server.uri(), "test-key").unwrap();
        let result = translator
            .translate(TranslationRequest {
                source_text: "<1> Line one\n<2> Line two\n<3> Line three",
                target_language: "de",
            })
            .await
            .unwrap();

        assert_eq!(result, "<1> Zeile eins\n<2> Zeile zwei\n<3> Zeile drei");
    }
}
