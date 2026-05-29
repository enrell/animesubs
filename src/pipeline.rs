use std::path::PathBuf;

use tempfile::tempdir;

use crate::error::{Result, SubtitleToolkitError};
use crate::media::mkv::{
    discover_mkv_files, extract_subtitle, inspect_mkv, mux_subtitle_in_place, select_ass_track,
};
use crate::subtitles::ass::AssSubtitle;
use crate::subtitles::structured::{apply_translation, parse_numbered_text, to_numbered_text};
use crate::translation::{TranslationRequest, Translator};

#[derive(Debug, Clone)]
pub struct TranslateMkvOptions {
    pub input: PathBuf,
    pub target_language: String,
    pub track_id: Option<u64>,
    pub keep_temp: bool,
}

pub async fn translate_mkv<T: Translator>(
    options: TranslateMkvOptions,
    translator: &T,
) -> Result<()> {
    let files = discover_mkv_files(&options.input).await?;

    for file in files {
        translate_one(file, &options, translator).await?;
    }

    Ok(())
}

async fn translate_one<T: Translator>(
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
    let mut ass = AssSubtitle::parse(&source)?;
    let numbered = to_numbered_text(ass.document());
    let ids = ass
        .document()
        .cues
        .iter()
        .map(|cue| cue.id)
        .collect::<Vec<_>>();

    let translated_text = translator
        .translate(TranslationRequest {
            source_text: &numbered,
            target_language: &options.target_language,
        })
        .await?;
    let parsed = parse_numbered_text(&translated_text, &ids)?;
    apply_translation(ass.document_mut(), parsed);

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
