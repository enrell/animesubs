use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{
    TranslationLimits, TranslationRequest, Translator, provider_http_error,
    provider_transport_error,
};

/// Translator backend that calls the [DeepL](https://www.deepl.com) `/v2/translate` endpoint.
///
/// Supports both the free tier (`https://api-free.deepl.com`) and the pro tier
/// (`https://api.deepl.com`). The default base URL targets the free tier.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> animesubs_engine::Result<()> {
/// use animesubs_engine::DeepLTranslator;
///
/// let translator = DeepLTranslator::new("your-api-key")?;
/// // let result = translator.translate(request).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct DeepLTranslator {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl DeepLTranslator {
    /// Create a new translator targeting the DeepL free API (`https://api-free.deepl.com`).
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        Self::with_base_url("https://api-free.deepl.com", api_key)
    }

    /// Create a new translator with a custom base URL.
    ///
    /// Use `"https://api.deepl.com"` for the pro tier, or
    /// `"https://api-free.deepl.com"` for the free tier (default).
    pub fn with_base_url(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
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
impl Translator for DeepLTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let expected_count = request.source_text.lines().count();
        let response = self
            .client
            .post(format!("{}/v2/translate", self.base_url))
            .header("Authorization", format!("DeepL-Auth-Key {}", self.api_key))
            .json(&DeepLTranslateRequest {
                text: request.source_text.lines().collect(),
                target_lang: &request.target_language.to_uppercase(),
                split_sentences: "0",
                preserve_formatting: true,
            })
            .send()
            .await
            .map_err(|error| provider_transport_error("deepl", error))?;

        if !response.status().is_success() {
            return Err(provider_http_error("deepl", response).await);
        }

        let body = response.json::<DeepLTranslateResponse>().await?;
        if body.translations.len() != expected_count {
            return Err(SubtitleToolkitError::InvalidTranslation {
                message: format!(
                    "deepl returned {} translations for {expected_count} requested items",
                    body.translations.len()
                ),
            });
        }
        let translated = body
            .translations
            .into_iter()
            .map(|t| t.text)
            .collect::<Vec<_>>()
            .join("\n");

        Ok(translated)
    }

    fn identifier(&self) -> String {
        format!("deepl:{}", self.base_url)
    }

    fn limits(&self) -> TranslationLimits {
        TranslationLimits {
            max_items: 200,
            max_request_bytes: 120 * 1024,
            reject_unchanged_output: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct DeepLTranslateRequest<'a> {
    text: Vec<&'a str>,
    target_lang: &'a str,
    split_sentences: &'static str,
    preserve_formatting: bool,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslateResponse {
    translations: Vec<DeepLTranslation>,
}

#[derive(Debug, Deserialize)]
struct DeepLTranslation {
    text: String,
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
            .and(path("/v2/translate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "translations": [{ "text": "<1> Olá" }, { "text": "<2> mundo" }]
            })))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "test-key").unwrap();
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
    async fn sends_deepl_auth_header() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(header("Authorization", "DeepL-Auth-Key my-secret-key"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "translations": [{ "text": "<1> ok" }]
            })))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "my-secret-key").unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "de",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn uppercases_target_language() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "translations": [{ "text": "<1> ok" }]
            })))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "test-key").unwrap();
        // "pt-BR" should be sent as "PT-BR" in the request body
        translator
            .translate(TranslationRequest {
                source_text: "<1> test",
                target_language: "pt-BR",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn error_on_non_200() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(403).set_body_string("Quota exceeded"))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "bad-key").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "de",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("deepl"));
        assert!(err.to_string().contains("Quota exceeded"));
    }

    #[tokio::test]
    async fn error_on_empty_translations() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "translations": []
            })))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "test-key").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "de",
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("0 translations for 1 requested"));
    }

    #[tokio::test]
    async fn sends_each_line_as_separate_array_element() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/v2/translate"))
            .and(wiremock::matchers::body_string_contains(r#""<1> hello""#))
            .and(wiremock::matchers::body_string_contains(r#""<2> world""#))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "translations": [{ "text": "<1> Olá" }, { "text": "<2> mundo" }]
            })))
            .expect(1)
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "test-key").unwrap();
        translator
            .translate(TranslationRequest {
                source_text: "<1> hello\n<2> world",
                target_language: "pt-BR",
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn joins_translated_lines_back() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "translations": [
                    { "text": "<1> Zeile eins" },
                    { "text": "<2> Zeile zwei" },
                    { "text": "<3> Zeile drei" }
                ]
            })))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "test-key").unwrap();
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
