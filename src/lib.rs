//! # psyche-subtitle-toolkit
//!
//! A toolkit for extracting, translating, and muxing ASS subtitles in MKV files.
//! Designed for Psyche but usable as a
//! standalone CLI or library.
//!
//! ## Quick start (library)
//!
//! ```no_run
//! use psyche_subtitle_toolkit::{translate_mkv, TranslateMkvOptions, OllamaTranslator};
//!
//! # async fn example() -> psyche_subtitle_toolkit::Result<()> {
//! let translator = OllamaTranslator::new("llama3.1");
//! translate_mkv(
//!     TranslateMkvOptions {
//!         input: "/media/anime/episode.mkv".into(),
//!         target_language: "pt-BR".into(),
//!         track_id: None,
//!         keep_temp: false,
//!     },
//!     &translator,
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
pub mod subtitles;
pub mod translation;

// Error types
pub use error::{Result, SubtitleToolkitError};

// MKV inspection
pub use media::mkv::{MkvInfo, MkvTrack, MkvTrackProperties, inspect_mkv, select_ass_track};

// Pipeline
pub use pipeline::{TranslateMkvOptions, translate_mkv};

// Subtitle model
pub use subtitles::ass::AssSubtitle;
pub use subtitles::model::{SubtitleCue, SubtitleDocument};
pub use subtitles::structured::{
    apply_translation, chunk_document, parse_numbered_text, reinject_tags, strip_tags,
    to_numbered_text,
};

// Translation
pub use translation::ollama::OllamaTranslator;
pub use translation::{TranslationRequest, Translator};
