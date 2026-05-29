//! # psyche-subtitle-toolkit
//!
//! A toolkit for extracting, translating, and muxing ASS subtitles in MKV files.
//! Designed for Psyche but usable as a
//! standalone CLI or library.
//!
//! ## Quick start (library)
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
//! ## Quick start (CLI)
//!
//! ```sh
//! psyche-subtitle-toolkit inspect episode.mkv
//! psyche-subtitle-toolkit translate --input episode.mkv --to pt-BR --model llama3.1
//! ```

#![forbid(unsafe_code)]

pub mod error;
pub mod media;
pub mod pipeline;
mod retry;
pub mod subtitles;
pub mod translation;

// Error types
pub use error::{Result, SubtitleToolkitError};

// MKV inspection
pub use media::mkv::{MkvInfo, MkvTrack, MkvTrackProperties, inspect_mkv, select_ass_track};

// Pipeline
pub use pipeline::{TranslateMkvOptions, dry_run_summary, translate_ass, translate_mkv};

// Subtitle model
pub use subtitles::ass::AssSubtitle;
pub use subtitles::model::{SubtitleCue, SubtitleDocument};
pub use subtitles::structured::{
    apply_translation, chunk_document, chunk_document_by_lines, parse_numbered_text,
    reinject_tags, strip_tags, to_numbered_text,
};

// Translation
pub use translation::deepl::DeepLTranslator;
pub use translation::gemini::GeminiTranslator;
pub use translation::google::GoogleTranslator;
pub use translation::ollama::OllamaTranslator;
pub use translation::openai::OpenAiTranslator;
pub use translation::openrouter::OpenRouterTranslator;
pub use translation::{TranslationRequest, Translator};
