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
    /// If `true`, shows what would be translated without modifying files.
    pub dry_run: bool,
    /// Optional source language hint (e.g. `"en"`, `"ja"`).
    pub source_language: Option<String>,
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

    if options.dry_run {
        let (clean_doc, _) = strip_tags(ass.document());
        let chunks = chunk_document(&clean_doc, 5000);
        let cue_count = clean_doc.cues.len();
        let total_chars: usize = clean_doc.cues.iter().map(|c| c.text.len()).sum();
        println!(
            "[dry-run] {}: {} cues, {} chars, {} chunk(s) → {}",
            file.display(),
            cue_count,
            total_chars,
            chunks.len(),
            options.target_language,
        );
        return Ok(());
    }

    let translated_ass = translate_ass(
        ass,
        &options.target_language,
        options.source_language.as_deref(),
        translator,
    )
    .await?;

    tokio::fs::write(&translated_path, translated_ass.render()).await?;
    mux_subtitle_in_place(&file, track.id, &translated_path, &options.target_language).await?;

    if options.keep_temp {
        let persisted = file.with_extension("psyche-subtitle-toolkit-temp");
        tokio::fs::create_dir_all(&persisted).await?;
        tokio::fs::copy(&extracted_path, persisted.join("source.ass")).await?;
        tokio::fs::copy(&translated_path, persisted.join("translated.ass")).await?;
    }

    Ok(())
}

/// Translate an ASS subtitle through the full processing pipeline.
///
/// This is the core subtitle processing function:
/// 1. Strips ASS override tags (`{\pos(...)}`, `{\an7}`, etc.)
/// 2. Chunks cues into batches (5000 chars each)
/// 3. Translates each chunk via the provided [`Translator`]
/// 4. Applies translated text back to the document
/// 5. Re-injects the original override tags
///
/// Returns the translated [`AssSubtitle`]. Use [`AssSubtitle::render`] to get
/// the final ASS string.
pub async fn translate_ass<T: Translator + ?Sized>(
    mut ass: AssSubtitle,
    target_language: &str,
    source_language: Option<&str>,
    translator: &T,
) -> Result<AssSubtitle> {
    let (mut clean_doc, tag_map) = strip_tags(ass.document());

    let chunks = chunk_document(&clean_doc, 5000);
    let mut all_translated = BTreeMap::new();

    for chunk in &chunks {
        let numbered = to_numbered_text(chunk);
        let ids = chunk.cues.iter().map(|cue| cue.id).collect::<Vec<_>>();

        let translated_text = translator
            .translate(TranslationRequest {
                source_text: &numbered,
                target_language,
                source_language,
            })
            .await?;
        let parsed = parse_numbered_text(&translated_text, &ids)?;
        all_translated.extend(parsed);
    }

    apply_translation(&mut clean_doc, all_translated);
    reinject_tags(&mut clean_doc, &tag_map);

    *ass.document_mut() = clean_doc;
    Ok(ass)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SubtitleToolkitError;
    use crate::subtitles::ass::AssSubtitle;
    use crate::translation::{TranslationRequest, Translator};
    use std::sync::Mutex;

    /// A mock translator for integration tests.
    ///
    /// Records each source text it receives and returns translations from
    /// a pre-configured map. If the map has no entry, returns a default
    /// identity translation (same text back).
    struct FakeTranslator {
        /// Source texts received, in call order.
        received: Mutex<Vec<String>>,
        /// Maps source text → translated text. If missing, returns source unchanged.
        responses: std::collections::HashMap<String, String>,
        /// If set, all calls return this error.
        error: Option<String>,
    }

    impl FakeTranslator {
        fn new(responses: std::collections::HashMap<String, String>) -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                responses,
                error: None,
            }
        }

        fn with_error(message: &str) -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                responses: std::collections::HashMap::new(),
                error: Some(message.to_string()),
            }
        }

        fn received_texts(&self) -> Vec<String> {
            self.received.lock().unwrap().clone()
        }
    }

    #[async_trait::async_trait]
    impl Translator for FakeTranslator {
        async fn translate(&self, request: TranslationRequest<'_>) -> crate::error::Result<String> {
            self.received
                .lock()
                .unwrap()
                .push(request.source_text.to_string());

            if let Some(msg) = &self.error {
                return Err(SubtitleToolkitError::Translation {
                    provider: "fake",
                    message: msg.clone(),
                });
            }

            Ok(self
                .responses
                .get(request.source_text)
                .cloned()
                .unwrap_or_else(|| request.source_text.to_string()))
        }
    }

    const SIMPLE_ASS: &str = r"[Script Info]
Title: Test
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,20

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello world
Dialogue: 0,0:00:03.00,0:00:04.00,Default,,0,0,0,,Goodbye world
";

    const ASS_WITH_TAGS: &str = r"[Script Info]
Title: Test Tags
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,20

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,{\pos(857.6,122.4)}{\an7}Status line
Dialogue: 0,0:00:03.00,0:00:04.00,Default,,0,0,0,,Normal text
";

    #[tokio::test]
    async fn pipeline_translates_dialogue_and_preserves_structure() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        let mut responses = std::collections::HashMap::new();
        responses.insert("<1> Hello world\n<2> Goodbye world".to_string(), "<1> Olá mundo\n<2> Adeus mundo".to_string());

        let translator = FakeTranslator::new(responses);
        let result = translate_ass(ass, "pt-BR", None, &translator).await.unwrap();
        let rendered = result.render();

        assert!(rendered.contains("Olá mundo"));
        assert!(rendered.contains("Adeus mundo"));
        assert!(!rendered.contains("Hello world"));
        assert!(!rendered.contains("Goodbye world"));

        // Headers and styles preserved
        assert!(rendered.contains("[Script Info]"));
        assert!(rendered.contains("[V4+ Styles]"));
        assert!(rendered.contains("[Events]"));
    }

    #[tokio::test]
    async fn pipeline_passes_numbered_text_and_target_language_to_translator() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        let mut responses = std::collections::HashMap::new();
        responses.insert("<1> Hello world\n<2> Goodbye world".to_string(), "<1> translated1\n<2> translated2".to_string());

        let translator = FakeTranslator::new(responses);
        translate_ass(ass, "ja", None, &translator).await.unwrap();

        let texts = translator.received_texts();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "<1> Hello world\n<2> Goodbye world");
    }

    #[tokio::test]
    async fn pipeline_strips_and_reinjects_override_tags() {
        let ass = AssSubtitle::parse(ASS_WITH_TAGS).unwrap();

        // The translator receives clean text (no tags)
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> Status line\n<2> Normal text".to_string(),
            "<1> Linha de status\n<2> Texto normal".to_string(),
        );

        let translator = FakeTranslator::new(responses);
        let result = translate_ass(ass, "pt-BR", None, &translator).await.unwrap();
        let rendered = result.render();

        // Tags reinjected
        assert!(rendered.contains(r"{\pos(857.6,122.4)}{\an7}Linha de status"));
        // Second line had no tags, stays clean
        assert!(rendered.contains("Texto normal"));
        assert!(!rendered.contains("Normal text"));

        // Verify translator received clean text (no tags)
        let texts = translator.received_texts();
        assert!(!texts[0].contains(r"{\pos"));
        assert!(!texts[0].contains(r"{\an7}"));
    }

    #[tokio::test]
    async fn pipeline_chunks_large_documents() {
        // Build an ASS with 100 cues. Each numbered line is ~60 chars,
        // so 100 lines ≈ 6000 chars — exceeds the 5000-char chunk limit.
        let mut lines = vec![
            "[Script Info]".to_string(),
            "Title: Big".to_string(),
            "ScriptType: v4.00+".to_string(),
            "".to_string(),
            "[V4+ Styles]".to_string(),
            "Format: Name, Fontname, Fontsize".to_string(),
            "Style: Default,Arial,20".to_string(),
            "".to_string(),
            "[Events]".to_string(),
            "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text"
                .to_string(),
        ];

        for i in 1..=100 {
            let text = format!("This is subtitle line number {i} with enough text to fill space");
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,{}",
                i, i + 1,
                text
            ));
        }

        let ass_content = lines.join("\n");

        // First, verify chunking actually splits at 5000 chars
        let ass = AssSubtitle::parse(&ass_content).unwrap();
        let (clean_doc, _) = crate::subtitles::structured::strip_tags(ass.document());
        let chunks = crate::subtitles::structured::chunk_document(&clean_doc, 5000);
        assert!(chunks.len() > 1, "100 cues should produce multiple chunks at 5000 chars");

        // Build a FakeTranslator that replaces "subtitle line" → "linha de legenda"
        // for any input. This works regardless of how the text is chunked.
        let translator = FakeTranslator::new(std::collections::HashMap::new());

        let ass = AssSubtitle::parse(&ass_content).unwrap();
        let result = translate_ass(ass, "pt-BR", None, &translator).await.unwrap();
        let rendered = result.render();

        // All 100 cues should be present (identity translation)
        for i in 1..=100 {
            assert!(
                rendered.contains(&format!("subtitle line number {i}")),
                "missing cue {i} in rendered output"
            );
        }

        // Translator was called multiple times (multiple chunks)
        let texts = translator.received_texts();
        assert!(texts.len() > 1, "expected multiple chunk calls, got {}", texts.len());

        // All 100 cue IDs accounted for across all calls
        let mut all_ids: Vec<usize> = Vec::new();
        for text in &texts {
            for line in text.lines() {
                if let Some(start) = line.find('<')
                    && let Some(end) = line[start + 1..].find('>')
                    && let Ok(id) = line[start + 1..start + 1 + end].parse::<usize>()
                {
                    all_ids.push(id);
                }
            }
        }
        all_ids.sort();
        assert_eq!(all_ids, (1..=100).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn pipeline_propagates_translator_error() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let translator = FakeTranslator::with_error("rate limit exceeded");

        let err = translate_ass(ass, "pt-BR", None, &translator).await.unwrap_err();

        assert!(err.to_string().contains("fake"));
        assert!(err.to_string().contains("rate limit exceeded"));
    }

    #[tokio::test]
    async fn pipeline_rejects_incomplete_translation() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        // Translator returns only one of two expected IDs
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> Hello world\n<2> Goodbye world".to_string(),
            "<1> Olá mundo".to_string(), // missing <2>
        );

        let translator = FakeTranslator::new(responses);
        let err = translate_ass(ass, "pt-BR", None, &translator).await.unwrap_err();

        assert!(err.to_string().contains("missing id <2>"));
    }

    #[tokio::test]
    async fn pipeline_handles_multiline_cues() {
        let ass_content = r"[Script Info]
Title: Multiline
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,20

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,First line\NSecond line
";

        let ass = AssSubtitle::parse(ass_content).unwrap();

        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> First line\\NSecond line".to_string(),
            "<1> Primeira linha\\NSegunda linha".to_string(),
        );

        let translator = FakeTranslator::new(responses);
        let result = translate_ass(ass, "pt-BR", None, &translator).await.unwrap();
        let rendered = result.render();

        assert!(rendered.contains("Primeira linha"));
        assert!(rendered.contains("Segunda linha"));
    }
}
