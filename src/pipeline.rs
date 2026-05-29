use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tempfile::tempdir;
use tokio::sync::Semaphore;

use crate::error::{Result, SubtitleToolkitError};
use crate::media::mkv::{
    SubtitleFormat, discover_mkv_files, extract_subtitle, inspect_mkv, mux_subtitle_in_place,
    select_subtitle_track,
};
use crate::retry::retry_async;
use crate::subtitles::ass::AssSubtitle;
use crate::subtitles::model::SubtitleDocument;
use crate::subtitles::srt::SrtSubtitle;
use crate::subtitles::structured::{
    apply_translation, chunk_document_by_lines, parse_numbered_text, reinject_tags, strip_tags,
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
    /// If `true`, saves progress to a file and skips already-translated files on restart.
    pub resume: bool,
    /// Maximum number of chunks to translate concurrently. Default: 1 (sequential).
    pub max_concurrent: usize,
}

/// Translate ASS subtitles in MKV file(s) and mux the result back in-place.
///
/// For each MKV file:
/// 1. Inspects tracks and selects the ASS subtitle track
/// 2. Extracts the subtitle to a temp directory
/// 3. Strips ASS override tags (`{\pos(...)}`, `{\an7}`, etc.)
/// 4. Chunks cues into 200-line batches
/// 5. Translates each chunk via the provided [`Translator`]
/// 6. Re-injects override tags
/// 7. Muxes the translated subtitle into the MKV (replacing the original track)
///
/// If `input` is a directory, processes all `.mkv` files sequentially.
pub async fn translate_mkv(
    options: TranslateMkvOptions,
    translator: Arc<dyn Translator>,
) -> Result<()> {
    let files = discover_mkv_files(&options.input).await?;

    let progress_path = progress_file_path(&options.input);
    let mut completed: Vec<String> = if options.resume && progress_path.exists() {
        let data = tokio::fs::read_to_string(&progress_path).await?;
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        Vec::new()
    };

    let total = files.len();
    for (i, file) in files.into_iter().enumerate() {
        let file_str = file.to_string_lossy().to_string();

        if options.resume && completed.contains(&file_str) {
            eprintln!("[resume] skipping ({}/{}): {}", i + 1, total, file.display());
            continue;
        }

        translate_one(file, &options, translator.clone()).await?;

        if options.resume {
            completed.push(file_str);
            let json = serde_json::to_string_pretty(&completed)?;
            tokio::fs::write(&progress_path, &json).await?;
            eprintln!("[resume] progress saved ({}/{})", completed.len(), total);
        }
    }

    if options.resume && progress_path.exists() {
        tokio::fs::remove_file(&progress_path).await?;
    }

    Ok(())
}

async fn translate_one(
    file: PathBuf,
    options: &TranslateMkvOptions,
    translator: Arc<dyn Translator>,
) -> Result<()> {
    let info = inspect_mkv(&file).await?;
    let (track, format) = select_subtitle_track(&info, options.track_id)
        .ok_or_else(|| SubtitleToolkitError::NoSubtitleTrack { path: file.clone() })?;

    let ext = match format {
        SubtitleFormat::Ass => "ass",
        SubtitleFormat::Srt => "srt",
    };

    let temp_dir = tempdir()?;
    let extracted_path = temp_dir.path().join(format!("source.{ext}"));
    let translated_path = temp_dir.path().join(format!("translated.{ext}"));

    eprintln!("[translate] {} ({})", file.display(), match format {
        SubtitleFormat::Ass => "ASS",
        SubtitleFormat::Srt => "SRT",
    });
    extract_subtitle(&file, track.id, &extracted_path).await?;

    let source = tokio::fs::read_to_string(&extracted_path).await?;

    let rendered = match format {
        SubtitleFormat::Ass => {
            let ass = AssSubtitle::parse(&source)?;
            if options.dry_run {
                let summary = dry_run_summary(ass.document(), &options.target_language);
                println!("[dry-run] {}: {}", file.display(), summary);
                return Ok(());
            }
            translate_ass(
                ass,
                &options.target_language,
                options.source_language.as_deref(),
                options.max_concurrent,
                translator,
            )
            .await?
            .render()
        }
        SubtitleFormat::Srt => {
            let srt = SrtSubtitle::parse(&source)?;
            if options.dry_run {
                let summary = dry_run_summary(srt.document(), &options.target_language);
                println!("[dry-run] {}: {}", file.display(), summary);
                return Ok(());
            }
            translate_srt(
                srt,
                &options.target_language,
                options.source_language.as_deref(),
                options.max_concurrent,
                translator,
            )
            .await?
            .render()
        }
    };

    eprintln!("[translate] muxing translated subtitle");
    tokio::fs::write(&translated_path, &rendered).await?;
    mux_subtitle_in_place(&file, track.id, &translated_path, &options.target_language).await?;
    eprintln!("[translate] done: {}", file.display());

    if options.keep_temp {
        let persisted = file.with_extension("psyche-subtitle-toolkit-temp");
        tokio::fs::create_dir_all(&persisted).await?;
        tokio::fs::copy(&extracted_path, persisted.join(format!("source.{ext}"))).await?;
        tokio::fs::copy(&translated_path, persisted.join(format!("translated.{ext}"))).await?;
    }

    Ok(())
}

fn progress_file_path(input: &std::path::Path) -> PathBuf {
    let dir = if input.is_dir() {
        input.to_path_buf()
    } else if input.extension().is_some() {
        input.parent().unwrap_or(input).to_path_buf()
    } else {
        input.to_path_buf()
    };
    dir.join(".psyche-subtitle-toolkit-progress.json")
}

/// Generate a dry-run summary for a subtitle document: cue count, char count, chunk count.
pub fn dry_run_summary(doc: &SubtitleDocument, target_language: &str) -> String {
    let (clean_doc, _) = strip_tags(doc);
    let chunks = chunk_document_by_lines(&clean_doc, 200);
    let cue_count = clean_doc.cues.len();
    let total_chars: usize = clean_doc.cues.iter().map(|c| c.text.len()).sum();
    format!(
        "{} cues, {} chars, {} chunk(s) → {}",
        cue_count,
        total_chars,
        chunks.len(),
        target_language,
    )
}

/// Translate an ASS subtitle through the full processing pipeline.
///
/// 1. Strips ASS override tags (`{\pos(...)}`, `{\an7}`, etc.)
/// 2. Chunks cues into 200-line batches
/// 3. Translates each chunk via the provided [`Translator`]
/// 4. Applies translated text back to the document
/// 5. Re-injects the original override tags
///
/// Returns the translated [`AssSubtitle`]. Use [`AssSubtitle::render`] to get
/// the final ASS string.
pub async fn translate_ass(
    mut ass: AssSubtitle,
    target_language: &str,
    source_language: Option<&str>,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<AssSubtitle> {
    let (mut clean_doc, tag_map) = strip_tags(ass.document());
    clean_doc = translate_document(clean_doc, target_language, source_language, max_concurrent, translator).await?;
    reinject_tags(&mut clean_doc, &tag_map);
    *ass.document_mut() = clean_doc;
    Ok(ass)
}

/// Translate an SRT subtitle through the processing pipeline.
///
/// 1. Chunks cues into 200-line batches
/// 2. Translates each chunk via the provided [`Translator`]
/// 3. Applies translated text back to the document
///
/// Returns the translated [`SrtSubtitle`]. Use [`SrtSubtitle::render`] to get
/// the final SRT string.
pub async fn translate_srt(
    mut srt: SrtSubtitle,
    target_language: &str,
    source_language: Option<&str>,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<SrtSubtitle> {
    let clean_doc = translate_document(
        srt.document().clone(),
        target_language,
        source_language,
        max_concurrent,
        translator,
    )
    .await?;
    *srt.document_mut() = clean_doc;
    Ok(srt)
}

/// Core translation logic shared by ASS and SRT pipelines.
async fn translate_document(
    mut doc: SubtitleDocument,
    target_language: &str,
    source_language: Option<&str>,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<SubtitleDocument> {
    let chunks = chunk_document_by_lines(&doc, 200);
    let chunk_count = chunks.len();
    let cue_count = doc.cues.len();
    let total_chars: usize = doc.cues.iter().map(|c| c.text.len()).sum();
    eprintln!(
        "[translate] {} cues, {} chars, {} chunk(s), {} concurrent",
        cue_count, total_chars, chunk_count, max_concurrent,
    );

    let semaphore = Arc::new(Semaphore::new(max_concurrent));
    let mut join_set = tokio::task::JoinSet::new();

    for (i, chunk) in chunks.into_iter().enumerate() {
        if chunk_count > 1 {
            let chunk_chars: usize = chunk.cues.iter().map(|c| c.text.len()).sum();
            eprintln!(
                "[translate] chunk {}/{}: {} cues, {} chars",
                i + 1,
                chunk_count,
                chunk.cues.len(),
                chunk_chars,
            );
        }
        let numbered = to_numbered_text(&chunk);
        let ids: Vec<usize> = chunk.cues.iter().map(|cue| cue.id).collect();
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|e| SubtitleToolkitError::Translation {
                provider: "pipeline",
                message: format!("semaphore closed: {e}"),
            })?;
        let translator = translator.clone();
        let target = target_language.to_string();
        let source = source_language.map(|s| s.to_string());

        join_set.spawn(async move {
            let _permit = permit;
            let numbered_clone = numbered.clone();
            let ids_clone = ids.clone();
            let result = retry_async(3, || {
                let numbered = numbered_clone.clone();
                let ids = ids_clone.clone();
                let translator = translator.clone();
                let target = target.clone();
                let source = source.clone();
                async move {
                    let translated_text = translator
                        .translate(TranslationRequest {
                            source_text: &numbered,
                            target_language: &target,
                            source_language: source.as_deref(),
                        })
                        .await?;
                    parse_numbered_text(&translated_text, &ids)
                }
            })
            .await;
            (i, result)
        });
    }

    let mut all_translated = BTreeMap::new();
    let mut results: Vec<(usize, Result<BTreeMap<usize, String>>)> = Vec::new();
    while let Some(result) = join_set.join_next().await {
        let (i, outcome) = result.map_err(|e| SubtitleToolkitError::Translation {
            provider: "pipeline",
            message: format!("task panicked: {e}"),
        })?;
        results.push((i, outcome));
    }
    results.sort_by_key(|(i, _)| *i);
    for (_, result) in results {
        all_translated.extend(result?);
    }

    apply_translation(&mut doc, all_translated);
    Ok(doc)
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
        /// If set, returns responses in order (first call → first response, etc.).
        /// Used for testing retry on malformed output.
        sequential: Mutex<Vec<String>>,
    }

    impl FakeTranslator {
        fn new(responses: std::collections::HashMap<String, String>) -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                responses,
                error: None,
                sequential: Mutex::new(Vec::new()),
            }
        }

        fn with_error(message: &str) -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                responses: std::collections::HashMap::new(),
                error: Some(message.to_string()),
                sequential: Mutex::new(Vec::new()),
            }
        }

        fn with_sequential_responses(responses: Vec<String>) -> Self {
            Self {
                received: Mutex::new(Vec::new()),
                responses: std::collections::HashMap::new(),
                error: None,
                sequential: Mutex::new(responses),
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

            // Sequential mode: pop from front of queue
            {
                let mut seq = self.sequential.lock().unwrap();
                if !seq.is_empty() {
                    return Ok(seq.remove(0));
                }
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

        let translator = Arc::new(FakeTranslator::new(responses));
        let result = translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>).await.unwrap();
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

        let translator = Arc::new(FakeTranslator::new(responses));
        translate_ass(ass, "ja", None, 1, translator.clone() as Arc<dyn Translator>).await.unwrap();

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

        let translator = Arc::new(FakeTranslator::new(responses));
        let result = translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>).await.unwrap();
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
        // Build an ASS with 300 cues to force multiple chunks at 200 lines/chunk.
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

        for i in 1..=300 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,subtitle line {i}",
                i, i + 1,
            ));
        }

        let ass_content = lines.join("\n");

        // Verify chunking splits at 200 lines
        let ass = AssSubtitle::parse(&ass_content).unwrap();
        let (clean_doc, _) = crate::subtitles::structured::strip_tags(ass.document());
        let chunks = crate::subtitles::structured::chunk_document_by_lines(&clean_doc, 200);
        assert_eq!(chunks.len(), 2, "300 cues should produce 2 chunks at 200 lines");
        assert_eq!(chunks[0].cues.len(), 200);
        assert_eq!(chunks[1].cues.len(), 100);

        // Build a FakeTranslator (identity translation)
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        let ass = AssSubtitle::parse(&ass_content).unwrap();
        let result = translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>).await.unwrap();
        let rendered = result.render();

        // All 300 cues should be present
        for i in 1..=300 {
            assert!(
                rendered.contains(&format!("subtitle line {i}")),
                "missing cue {i} in rendered output"
            );
        }

        // Translator was called twice (2 chunks)
        let texts = translator.received_texts();
        assert_eq!(texts.len(), 2);

        // All 300 cue IDs accounted for across all calls
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
        assert_eq!(all_ids, (1..=300).collect::<Vec<_>>());
    }

    #[tokio::test]
    async fn pipeline_propagates_translator_error() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let translator = FakeTranslator::with_error("rate limit exceeded");

        let err = translate_ass(ass, "pt-BR", None, 1, Arc::new(translator) as Arc<dyn Translator>).await.unwrap_err();

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
        let err = translate_ass(ass, "pt-BR", None, 1, Arc::new(translator) as Arc<dyn Translator>).await.unwrap_err();

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

        let translator = Arc::new(FakeTranslator::new(responses));
        let result = translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>).await.unwrap();
        let rendered = result.render();

        assert!(rendered.contains("Primeira linha"));
        assert!(rendered.contains("Segunda linha"));
    }

    #[tokio::test]
    async fn pipeline_passes_source_language_to_translator() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> Hello world\n<2> Goodbye world".to_string(),
            "<1> translated1\n<2> translated2".to_string(),
        );

        let translator = Arc::new(FakeTranslator::new(responses));
        translate_ass(ass, "pt-BR", Some("en"), 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();

        // Verify the translator was called (source_language flows through without breaking)
        let texts = translator.received_texts();
        assert_eq!(texts.len(), 1);
    }

    #[tokio::test]
    async fn pipeline_works_with_source_language_none() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> Hello world\n<2> Goodbye world".to_string(),
            "<1> translated1\n<2> translated2".to_string(),
        );

        let translator = Arc::new(FakeTranslator::new(responses));
        translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();

        let texts = translator.received_texts();
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn dry_run_summary_reports_cues_chars_chunks() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let summary = dry_run_summary(ass.document(), "pt-BR");

        assert!(summary.contains("2 cues"), "expected '2 cues' in: {summary}");
        assert!(summary.contains("1 chunk(s)"), "expected '1 chunk(s)' in: {summary}");
        assert!(summary.contains("→ pt-BR"), "expected '→ pt-BR' in: {summary}");
    }

    #[test]
    fn dry_run_summary_counts_chars() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let summary = dry_run_summary(ass.document(), "en");

        // "Hello world" = 11 chars, "Goodbye world" = 13 chars = 24 total
        assert!(summary.contains("24 chars"), "expected '24 chars' in: {summary}");
    }

    #[test]
    fn dry_run_summary_handles_empty_document() {
        let ass_content = r"[Script Info]
Title: Empty
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize
Style: Default,Arial,20

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
";
        let ass = AssSubtitle::parse(ass_content).unwrap();
        let summary = dry_run_summary(ass.document(), "de");

        assert!(summary.contains("0 cues"), "expected '0 cues' in: {summary}");
        assert!(summary.contains("0 chars"), "expected '0 chars' in: {summary}");
        assert!(summary.contains("0 chunk(s)"), "expected '0 chunk(s)' in: {summary}");
    }

    #[test]
    fn dry_run_summary_splits_large_documents() {
        // Build ASS with 100 cues to force multiple chunks
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
        for i in 1..=300 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,This is subtitle line number {i} with enough text to fill space",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let summary = dry_run_summary(ass.document(), "pt-BR");

        assert!(summary.contains("300 cues"), "expected '300 cues' in: {summary}");
        assert!(
            summary.contains("2 chunk(s)"),
            "expected '2 chunk(s)' in: {summary}"
        );
    }

    #[tokio::test]
    async fn pipeline_retries_chunk_on_malformed_output() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        // First call: returns broken output (missing <2>)
        // Second call: returns correct output
        let translator = Arc::new(FakeTranslator::with_sequential_responses(vec![
            "<1> Olá mundo".to_string(),           // missing <2>
            "<1> Olá mundo\n<2> Adeus mundo".to_string(), // correct
        ]));

        let result = translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>).await.unwrap();
        let rendered = result.render();

        assert!(rendered.contains("Olá mundo"));
        assert!(rendered.contains("Adeus mundo"));

        // Should have been called twice (1 failed + 1 success)
        let texts = translator.received_texts();
        assert_eq!(texts.len(), 2);
    }

    #[tokio::test]
    async fn pipeline_gives_up_after_repeated_malformed_output() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        // Always returns broken output (missing <2>)
        let translator = Arc::new(FakeTranslator::with_sequential_responses(vec![
            "<1> Olá mundo".to_string(),
            "<1> Olá mundo".to_string(),
            "<1> Olá mundo".to_string(),
            "<1> Olá mundo".to_string(), // 4 attempts total (1 initial + 3 retries)
        ]));

        let err = translate_ass(ass, "pt-BR", None, 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("missing id <2>"));

        // Should have been called 4 times (1 initial + 3 retries)
        let texts = translator.received_texts();
        assert_eq!(texts.len(), 4);
    }

    #[test]
    fn progress_file_path_for_directory() {
        let path = progress_file_path(std::path::Path::new("/media/anime"));
        assert_eq!(
            path,
            std::path::PathBuf::from("/media/anime/.psyche-subtitle-toolkit-progress.json")
        );
    }

    #[test]
    fn progress_file_path_for_file() {
        let path = progress_file_path(std::path::Path::new("/media/anime/episode.mkv"));
        assert_eq!(
            path,
            std::path::PathBuf::from("/media/anime/.psyche-subtitle-toolkit-progress.json")
        );
    }

    #[tokio::test]
    async fn progress_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");

        let completed = vec![
            "/media/anime/ep1.mkv".to_string(),
            "/media/anime/ep2.mkv".to_string(),
        ];
        let json = serde_json::to_string_pretty(&completed).unwrap();
        tokio::fs::write(&progress_path, &json).await.unwrap();

        let data = tokio::fs::read_to_string(&progress_path).await.unwrap();
        let loaded: Vec<String> = serde_json::from_str(&data).unwrap();

        assert_eq!(loaded, completed);
    }

    #[tokio::test]
    async fn progress_file_handles_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");

        // Should return empty vec when file doesn't exist
        let completed: Vec<String> = if progress_path.exists() {
            let data = tokio::fs::read_to_string(&progress_path).await.unwrap();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Vec::new()
        };

        assert!(completed.is_empty());
    }

    #[tokio::test]
    async fn progress_file_handles_corrupted_json() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");

        tokio::fs::write(&progress_path, "not valid json")
            .await
            .unwrap();

        let data = tokio::fs::read_to_string(&progress_path).await.unwrap();
        let completed: Vec<String> = serde_json::from_str(&data).unwrap_or_default();

        // Should fall back to empty vec on corrupt JSON
        assert!(completed.is_empty());
    }

    #[tokio::test]
    async fn pipeline_translates_concurrently() {
        // Build ASS with 300 cues → 2 chunks at 200 lines/chunk
        let mut lines = vec![
            "[Script Info]".to_string(),
            "Title: Concurrent".to_string(),
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
        for i in 1..=300 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,line {i}",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        // Translate with max_concurrent=2 (both chunks in parallel)
        let result = translate_ass(
            ass,
            "pt-BR",
            None,
            2,
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        let rendered = result.render();
        for i in 1..=300 {
            assert!(rendered.contains(&format!("line {i}")), "missing cue {i}");
        }

        // Both chunks should have been translated
        let texts = translator.received_texts();
        assert_eq!(texts.len(), 2);
    }

    #[tokio::test]
    async fn concurrent_translation_preserves_all_cues() {
        // Stress test: 1000 cues, 5 chunks, max_concurrent=5
        let mut lines = vec![
            "[Script Info]".to_string(),
            "Title: Stress".to_string(),
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
        for i in 1..=1000 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,stress line {i}",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        let result = translate_ass(
            ass,
            "pt-BR",
            None,
            5,
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        let rendered = result.render();
        // Every single cue must be present
        for i in 1..=1000 {
            assert!(
                rendered.contains(&format!("stress line {i}")),
                "missing cue {i} under concurrent translation"
            );
        }

        // 1000 cues / 200 lines per chunk = 5 chunks
        let texts = translator.received_texts();
        assert_eq!(texts.len(), 5, "expected 5 chunk calls, got {}", texts.len());
    }

    #[tokio::test]
    async fn concurrent_translation_output_is_deterministic() {
        // Run the same document through concurrent translation twice.
        // Output must be identical regardless of task scheduling.
        let make_ass = || {
            let mut lines = vec![
                "[Script Info]".to_string(),
                "Title: Deterministic".to_string(),
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
            for i in 1..=500 {
                lines.push(format!(
                    "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,det line {i}",
                    i, i + 1,
                ));
            }
            AssSubtitle::parse(&lines.join("\n")).unwrap()
        };

        let t1 = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));
        let r1 = translate_ass(
            make_ass(),
            "pt-BR",
            None,
            3,
            t1.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        let t2 = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));
        let r2 = translate_ass(
            make_ass(),
            "pt-BR",
            None,
            3,
            t2.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        assert_eq!(r1.render(), r2.render(), "concurrent output is non-deterministic");
    }

    #[tokio::test]
    async fn concurrent_error_propagates_correctly() {
        // First chunk succeeds, second chunk always fails.
        // The pipeline should return an error, not silently succeed.
        let mut lines = vec![
            "[Script Info]".to_string(),
            "Title: ErrProp".to_string(),
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
        for i in 1..=400 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,line {i}",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();

        // Always error — all chunks will fail
        let translator = Arc::new(FakeTranslator::with_error("provider down"));

        let err = translate_ass(
            ass,
            "pt-BR",
            None,
            3,
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap_err();

        assert!(err.to_string().contains("provider down"));
    }

    /// A translator that tracks the maximum number of concurrent translate calls.
    /// Used to verify the semaphore actually bounds concurrency.
    struct ConcurrencyTrackingTranslator {
        active: std::sync::atomic::AtomicU32,
        max_observed: std::sync::atomic::AtomicU32,
        received: Mutex<Vec<String>>,
    }

    impl ConcurrencyTrackingTranslator {
        fn new() -> Self {
            Self {
                active: std::sync::atomic::AtomicU32::new(0),
                max_observed: std::sync::atomic::AtomicU32::new(0),
                received: Mutex::new(Vec::new()),
            }
        }

        fn max_concurrent_calls(&self) -> u32 {
            self.max_observed.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[async_trait::async_trait]
    impl Translator for ConcurrencyTrackingTranslator {
        async fn translate(&self, request: TranslationRequest<'_>) -> crate::error::Result<String> {
            self.received.lock().unwrap().push(request.source_text.to_string());

            let current = self.active.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            // Update max_observed
            self.max_observed.fetch_max(current, std::sync::atomic::Ordering::SeqCst);

            // Simulate work — yield to let other tasks run
            tokio::task::yield_now().await;

            self.active.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

            // Identity translation
            Ok(request.source_text.to_string())
        }
    }

    #[tokio::test]
    async fn semaphore_bounds_concurrency() {
        // 600 cues → 3 chunks at 200 lines/chunk, max_concurrent=2
        // The semaphore should prevent all 3 from running simultaneously.
        let mut lines = vec![
            "[Script Info]".to_string(),
            "Title: Semaphore".to_string(),
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
        for i in 1..=600 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,sem line {i}",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(ConcurrencyTrackingTranslator::new());

        let result = translate_ass(
            ass,
            "pt-BR",
            None,
            2, // max_concurrent=2
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        // All 600 cues should be present
        let rendered = result.render();
        for i in 1..=600 {
            assert!(rendered.contains(&format!("sem line {i}")), "missing cue {i}");
        }

        // The max observed concurrency should be <= 2
        let max = translator.max_concurrent_calls();
        assert!(
            max <= 2,
            "semaphore failed to bound concurrency: observed {max} concurrent calls (expected <= 2)"
        );

        // All 3 chunks should have been called
        let texts = translator.received.lock().unwrap();
        assert_eq!(texts.len(), 3);
    }

    #[tokio::test]
    async fn sequential_mode_is_deterministic() {
        // With max_concurrent=1, received_texts() must be in spawn order.
        let mut lines = vec![
            "[Script Info]".to_string(),
            "Title: Seq".to_string(),
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
        for i in 1..=500 {
            lines.push(format!(
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,seq line {i}",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        translate_ass(
            ass,
            "pt-BR",
            None,
            1, // sequential
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        let texts = translator.received_texts();
        assert_eq!(texts.len(), 3);

        // In sequential mode, chunk 1 should be called before chunk 2, etc.
        // Verify by checking that each text starts with the expected cue ID range.
        assert!(texts[0].starts_with("<1> "), "first chunk should start with <1>");
        assert!(texts[1].starts_with("<201> "), "second chunk should start with <201>");
        assert!(texts[2].starts_with("<401> "), "third chunk should start with <401>");
    }
}
