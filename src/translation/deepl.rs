use serde::{Deserialize, Serialize};

use crate::error::{Result, SubtitleToolkitError};

use super::{TranslationRequest, Translator};

/// Translator backend that calls the [DeepL](https://www.deepl.com) `/v2/translate` endpoint.
///
/// Supports both the free tier (`https://api-free.deepl.com`) and the pro tier
/// (`https://api.deepl.com`). The default base URL targets the free tier.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> psyche_subtitle_toolkit::Result<()> {
/// use psyche_subtitle_toolkit::DeepLTranslator;
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
impl Translator for DeepLTranslator {
    async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
        let response = self
            .client
            .post(format!("{}/v2/translate", self.base_url))
            .header(
                "Authorization",
                format!("DeepL-Auth-Key {}", self.api_key),
            )
            .json(&DeepLTranslateRequest {
                text: vec![request.source_text],
                target_lang: &request.target_language.to_uppercase(),
                source_lang: request.source_language.map(|l| l.to_uppercase()),
                split_sentences: "0",
                preserve_formatting: true,
            })
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SubtitleToolkitError::Translation {
                provider: "deepl",
                message: response
                    .text()
                    .await
                    .unwrap_or_else(|_| "request failed".into()),
            });
        }

        let body = response.json::<DeepLTranslateResponse>().await?;
        let translated = body
            .translations
            .into_iter()
            .next()
            .ok_or_else(|| SubtitleToolkitError::Translation {
                provider: "deepl",
                message: "response contained no translations".into(),
            })?
            .text;

        Ok(translated)
    }
}

#[derive(Debug, Serialize)]
struct DeepLTranslateRequest<'a> {
    text: Vec<&'a str>,
    target_lang: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_lang: Option<String>,
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
                "translations": [{ "text": "<1> Olá\n<2> mundo" }]
            })))
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "test-key").unwrap();
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
            source_language: None,
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
            source_language: None,
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn error_on_non_200() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(403).set_body_string("Quota exceeded"),
            )
            .mount(&server)
            .await;

        let translator = DeepLTranslator::with_base_url(server.uri(), "bad-key").unwrap();
        let err = translator
            .translate(TranslationRequest {
                source_text: "<1> hello",
                target_language: "de",
            source_language: None,
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
            source_language: None,
            })
            .await
            .unwrap_err();

        assert!(err.to_string().contains("no translations"));
    }
}
