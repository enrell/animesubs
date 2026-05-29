use std::collections::BTreeMap;
use std::path::PathBuf;

use tempfile::tempdir;

use crate::error::{Result, SubtitleToolkitError};
use crate::media::mkv::{
    discover_mkv_files, extract_subtitle, inspect_mkv, mux_subtitle_in_place, select_ass_track,
};
use crate::subtitles::ass::AssSubtitle;
use crate::subtitles::structured::{
    apply_translation, chunk_document, parse_numbered_text, reinject_tags, strip_tags,
    to_numbered_text,
};
use crate::translation::{TranslationRequest, Translator};

/// Options for the [`translate_mkv`] pipeline.
#[derive(Debug, Clone)]
pub struct TranslateMkvOptions {
    /// Path to an MKV file or directory containing MKV files.
    pub input: PathBuf,
    /// Target language code (e.g. `"pt-BR"`, `"en"`, `"ja"`).
    pub target_language: String,
    /// Specific subtitle track ID to translate. If `None`, selects the first ASS track.
    pub track_id: Option<u64>,
    /// If `true`, preserves extracted/translated ASS files alongside the MKV.
    pub keep_temp: bool,
}

/// Translate ASS subtitles in MKV file(s) and mux the result back in-place.
///
/// For each MKV file:
/// 1. Inspects tracks and selects the ASS subtitle track
/// 2. Extracts the subtitle to a temp directory
/// 3. Strips ASS override tags (`{\pos(...)}`, `{\an7}`, etc.)
/// 4. Chunks cues into ~5000-char batches
/// 5. Translates each chunk via the provided [`Translator`]
/// 6. Re-injects override tags
/// 7. Muxes the translated subtitle into the MKV (replacing the original track)
///
/// If `input` is a directory, processes all `.mkv` files sequentially.
pub async fn translate_mkv<T: Translator + ?Sized>(
    options: TranslateMkvOptions,
    translator: &T,
) -> Result<()> {
    let files = discover_mkv_files(&options.input).await?;

    for file in files {
        translate_one(file, &options, translator).await?;
    }

    Ok(())
}

async fn translate_one<T: Translator + ?Sized>(
    file: PathBuf,
    options: &TranslateMkvOptions,
    translator: &T,
) -> Result<()> {
    let info = inspect_mkv(&file).await?;
    let track = select_ass_track(&info, options.track_id)
        .ok_or_else(|| SubtitleToolkitError::NoAssTrack { path: file.clone() })?;

    let temp_dir = tempdir()?;
    let extracted_path = temp_dir.path().join("source.ass");
    let translated_path = temp_dir.path().join("translated.ass");

    extract_subtitle(&file, track.id, &extracted_path).await?;

    let source = tokio::fs::read_to_string(&extracted_path).await?;
    let ass = AssSubtitle::parse(&source)?;
    let (mut clean_doc, tag_map) = strip_tags(ass.document());

    let chunks = chunk_document(&clean_doc, 5000);
    let mut all_translated = BTreeMap::new();

    for chunk in &chunks {
        let numbered = to_numbered_text(chunk);
        let ids = chunk.cues.iter().map(|cue| cue.id).collect::<Vec<_>>();

        let translated_text = translator
            .translate(TranslationRequest {
                source_text: &numbered,
                target_language: &options.target_language,
            })
            .await?;
        let parsed = parse_numbered_text(&translated_text, &ids)?;
        all_translated.extend(parsed);
    }

    apply_translation(&mut clean_doc, all_translated);
    reinject_tags(&mut clean_doc, &tag_map);

    let mut ass = ass;
    *ass.document_mut() = clean_doc;

    tokio::fs::write(&translated_path, ass.render()).await?;
    mux_subtitle_in_place(&file, track.id, &translated_path, &options.target_language).await?;

    if options.keep_temp {
        let persisted = file.with_extension("psyche-subtitle-toolkit-temp");
        tokio::fs::create_dir_all(&persisted).await?;
        tokio::fs::copy(&extracted_path, persisted.join("source.ass")).await?;
        tokio::fs::copy(&translated_path, persisted.join("translated.ass")).await?;
    }

    Ok(())
}
