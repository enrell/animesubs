//! # psyche-subtitle-toolkit
//!
//! Extract, translate, and mux ASS subtitles in MKV files.
//! Built for Psyche but usable as a standalone CLI or library.
//!
//! No cloud required. No telemetry. Every translation provider is opt-in.
//!
//! ## Supported providers
//!
//! | Provider | Struct | Endpoint |
//! |----------|--------|----------|
//! | [Ollama](https://ollama.com) | [`OllamaTranslator`] | `/api/generate` |
//! | [OpenAI](https://platform.openai.com) | [`OpenAiTranslator`] | `/v1/chat/completions` |
//! | [OpenRouter](https://openrouter.ai) | [`OpenRouterTranslator`] | `/api/v1/chat/completions` |
//! | [Anthropic](https://docs.anthropic.com) | [`AnthropicTranslator`] | `/v1/messages` |
//! | [DeepL](https://www.deepl.com) | [`DeepLTranslator`] | `/v2/translate` |
//! | [Google Translate](https://cloud.google.com/translate) | [`GoogleTranslator`] | `/language/translate/v2` |
//! | [Gemini](https://ai.google.dev/gemini-api/docs) | [`GeminiTranslator`] | `v1beta/models/{model}:generateContent` |
//!
//! ## Quick start (library)
//!
//! Translate an MKV file in-place:
//!
//! ```no_run
//! use std::sync::Arc;
//! use psyche_subtitle_toolkit::{translate_mkv, TranslateMkvOptions, OllamaTranslator, Translator};
//!
//! # async fn example() -> psyche_subtitle_toolkit::Result<()> {
//! let translator: Arc<dyn Translator> = Arc::new(OllamaTranslator::new("llama3.1")?);
//! translate_mkv(
//!     TranslateMkvOptions {
//!         input: "/media/anime/episode.mkv".into(),
//!         target_language: "pt-BR".into(),
//!         track_id: None,
//!         keep_temp: false,
//!         dry_run: false,
//!         source_language: None,
//!         resume: false,
//!         max_concurrent: 1,
//!     },
//!     translator,
//! ).await?;
//! # Ok(())
//! # }
//! ```
//!
//! Translate ASS content directly (no MKV I/O):
//!
//! ```no_run
//! use std::sync::Arc;
//! use psyche_subtitle_toolkit::{translate_ass, AssSubtitle, OllamaTranslator, Translator};
//!
//! # async fn example() -> psyche_subtitle_toolkit::Result<()> {
//! let ass = AssSubtitle::parse(&std::fs::read_to_string("source.ass")?)?;
//! let translator: Arc<dyn Translator> = Arc::new(OllamaTranslator::new("llama3.1")?);
//! let translated = translate_ass(ass, "pt-BR", Some("en"), 1, translator).await?;
//! std::fs::write("translated.ass", translated.render())?;
//! # Ok(())
//! # }
//! ```
//!
//! Implement a custom provider:
//!
//! ```
//! use async_trait::async_trait;
//! use psyche_subtitle_toolkit::{Translator, TranslationRequest, Result};
//!
//! struct MyTranslator;
//!
//! #[async_trait]
//! impl Translator for MyTranslator {
//!     async fn translate(&self, request: TranslationRequest<'_>) -> Result<String> {
//!         // Your translation logic here.
//!         // request.source_text is numbered: "<1> hello\n<2> world"
//!         // Return translated text in the same format.
//!         Ok(request.source_text.to_string())
//!     }
//! }
//! ```
//!
//! ## Pipeline overview
//!
//! 1. **Inspect** — `mkvmerge -J` identifies tracks, selects the ASS subtitle
//! 2. **Extract** — `mkvextract tracks` pulls the ASS file to a temp directory
//! 3. **Parse** — ASS parser reads dialogue lines, preserving headers and styles
//! 4. **Strip tags** — Override tags (`{\pos(...)}`, `{\an7}`) removed and stored
//! 5. **Chunk** — Cues split into 200-line batches
//! 6. **Translate** — Each chunk sent to the provider (concurrent if `max_concurrent > 1`)
//! 7. **Retry** — Failed chunks retried up to 3 times with exponential backoff
//! 8. **Apply** — Translated text mapped back to cues by ID
//! 9. **Reinject tags** — Original override tags prepended back
//! 10. **Mux** — `mkvmerge` replaces the original subtitle track in-place

#![forbid(unsafe_code)]

/// Error types for the subtitle toolkit.
pub mod error;
/// MKV container inspection and manipulation (mkvmerge/mkvextract wrappers).
pub mod media;
/// Translation pipeline: MKV full-pipeline and subtitle-only translation.
pub mod pipeline;
mod retry;
/// Subtitle parsing, chunking, and tag manipulation.
pub mod subtitles;
/// Pluggable translation providers and the [`Translator`] trait.
pub mod translation;

// Error types
pub use error::{Result, SubtitleToolkitError};

// MKV inspection
pub use media::mkv::{
    MkvInfo, MkvTrack, MkvTrackProperties, SubtitleFormat, inspect_mkv, select_ass_track,
    select_subtitle_track,
};

// Pipeline
pub use pipeline::{
    TranslateMkvOptions, dry_run_summary, translate_ass, translate_mkv, translate_srt,
    translate_vtt,
};

// Subtitle model
pub use subtitles::ass::AssSubtitle;
pub use subtitles::srt::SrtSubtitle;
pub use subtitles::vtt::VttSubtitle;
pub use subtitles::model::{SubtitleCue, SubtitleDocument};
pub use subtitles::structured::{
    apply_translation, chunk_document, chunk_document_by_lines, parse_numbered_text,
    reinject_tags, strip_tags, to_numbered_text,
};

// Translation providers
pub use translation::anthropic::AnthropicTranslator;
pub use translation::deepl::DeepLTranslator;
pub use translation::gemini::GeminiTranslator;
pub use translation::google::GoogleTranslator;
pub use translation::ollama::OllamaTranslator;
pub use translation::openai::OpenAiTranslator;
pub use translation::openrouter::OpenRouterTranslator;
pub use translation::{TranslationRequest, Translator};
