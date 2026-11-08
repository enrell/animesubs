//! Build engine translators from an API [`ProviderConfig`].

use std::sync::Arc;

use animesubs_api::{ProviderConfig, ProviderKind, error_codes};
use psyche_subtitle_toolkit::{
    AnthropicTranslator, DeepLTranslator, GeminiTranslator, GoogleTranslator, OllamaTranslator,
    OpenAiTranslator, OpenCodeGoTranslator, OpenCodeZenTranslator, OpenRouterTranslator,
    Translator,
};

use crate::ops::OpError;

/// Generic key variable honoured for every provider.
pub const API_KEY_ENV: &str = "ANIMESUBS_API_KEY";
/// Variable read by the standalone toolkit CLI; still honoured for compatibility.
pub const LEGACY_API_KEY_ENV: &str = "PSYCHE_SUBTITLE_API_KEY";

/// Key from the request, else `ANIMESUBS_API_KEY`, the provider-specific
/// variable, or `PSYCHE_SUBTITLE_API_KEY`, in that order.
pub fn resolve_api_key(config: &ProviderConfig) -> Option<String> {
    let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
    config
        .api_key
        .clone()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| env(API_KEY_ENV))
        .or_else(|| config.kind.api_key_env().and_then(env))
        .or_else(|| env(LEGACY_API_KEY_ENV))
}

/// Build a translator. Fails on missing model or key; never touches the network.
pub fn build_translator(config: &ProviderConfig) -> Result<Arc<dyn Translator>, OpError> {
    let base_url = config
        .base_url
        .as_deref()
        .filter(|url| !url.trim().is_empty());
    let model = || {
        config
            .model
            .clone()
            .filter(|model| !model.trim().is_empty())
            .ok_or_else(|| {
                OpError::bad_request(format!("provider `{}` needs a model", config.kind))
            })
    };
    let key = || {
        resolve_api_key(config).ok_or_else(|| {
            let hint = config
                .kind
                .api_key_env()
                .map(|name| format!(" or set {API_KEY_ENV} / {name}"))
                .unwrap_or_default();
            OpError::bad_request(format!("provider `{}` needs an API key{hint}", config.kind))
        })
    };

    let translator: Arc<dyn Translator> = match (config.kind, base_url) {
        (ProviderKind::Ollama, None) => Arc::new(OllamaTranslator::new(model()?).map_err(engine)?),
        (ProviderKind::Ollama, Some(url)) => {
            Arc::new(OllamaTranslator::with_base_url(url, model()?).map_err(engine)?)
        }
        (ProviderKind::Openai, None) => {
            Arc::new(OpenAiTranslator::new(key()?, model()?).map_err(engine)?)
        }
        // Local OpenAI-compatible servers (LM Studio, llama.cpp) take any key.
        (ProviderKind::Openai, Some(url)) => Arc::new(
            OpenAiTranslator::with_base_url(
                url,
                resolve_api_key(config).unwrap_or_default(),
                model()?,
            )
            .map_err(engine)?,
        ),
        (ProviderKind::OpencodeZen, None) => {
            Arc::new(OpenCodeZenTranslator::new(key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::OpencodeZen, Some(url)) => {
            Arc::new(OpenCodeZenTranslator::with_base_url(url, key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::OpencodeGo, None) => {
            Arc::new(OpenCodeGoTranslator::new(key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::OpencodeGo, Some(url)) => {
            Arc::new(OpenCodeGoTranslator::with_base_url(url, key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::Openrouter, None) => {
            Arc::new(OpenRouterTranslator::new(key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::Openrouter, Some(url)) => {
            Arc::new(OpenRouterTranslator::with_base_url(url, key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::Anthropic, None) => {
            Arc::new(AnthropicTranslator::new(key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::Anthropic, Some(url)) => {
            Arc::new(AnthropicTranslator::with_base_url(url, key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::Deepl, None) => Arc::new(DeepLTranslator::new(key()?).map_err(engine)?),
        (ProviderKind::Deepl, Some(url)) => {
            Arc::new(DeepLTranslator::with_base_url(url, key()?).map_err(engine)?)
        }
        (ProviderKind::Google, None) => Arc::new(GoogleTranslator::new(key()?).map_err(engine)?),
        (ProviderKind::Google, Some(url)) => {
            Arc::new(GoogleTranslator::with_base_url(url, key()?).map_err(engine)?)
        }
        (ProviderKind::Gemini, None) => {
            Arc::new(GeminiTranslator::new(key()?, model()?).map_err(engine)?)
        }
        (ProviderKind::Gemini, Some(url)) => {
            Arc::new(GeminiTranslator::with_base_url(url, key()?, model()?).map_err(engine)?)
        }
    };
    Ok(translator)
}

fn engine(error: psyche_subtitle_toolkit::SubtitleToolkitError) -> OpError {
    OpError::new(500, error_codes::INTERNAL, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(kind: ProviderKind) -> ProviderConfig {
        ProviderConfig {
            kind,
            model: Some("some-model".into()),
            base_url: Some("http://127.0.0.1:1".into()),
            api_key: Some("test-key".into()),
        }
    }

    #[test]
    fn every_provider_kind_builds_with_model_and_key() {
        for kind in ProviderKind::ALL {
            let translator = build_translator(&config(kind)).unwrap();
            assert!(
                !translator.identifier().contains("test-key"),
                "{kind}: key leaked"
            );
        }
    }

    #[test]
    fn llm_providers_require_a_model() {
        let mut ollama = config(ProviderKind::Ollama);
        ollama.model = None;
        let error = build_translator(&ollama).err().unwrap();
        assert_eq!(error.code, error_codes::BAD_REQUEST);

        let mut deepl = config(ProviderKind::Deepl);
        deepl.model = None;
        assert!(build_translator(&deepl).is_ok());
    }

    #[test]
    fn openai_compatible_local_endpoint_needs_no_key() {
        let mut local = config(ProviderKind::Openai);
        local.api_key = None;
        assert!(build_translator(&local).is_ok());
    }
}
