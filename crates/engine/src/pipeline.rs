use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::sync::Semaphore;

use crate::error::{Result, SubtitleToolkitError};
use crate::media::mkv::{
    SubtitleFormat, discover_mkv_files, extract_subtitle, inspect_mkv, mux_subtitle_in_place,
    select_subtitle_track,
};
use crate::ocr::{OcrConfig, ocr_pgs_to_srt};
use crate::retry::retry_async;
use crate::subtitles::ass::AssSubtitle;
use crate::subtitles::model::SubtitleDocument;
use crate::subtitles::srt::SrtSubtitle;
use crate::subtitles::structured::{
    apply_translation, chunk_document_with_limits, parse_numbered_text, protect_ass_spans,
    protect_markup_spans, restore_protected_spans, strip_tags, to_numbered_text,
};
use crate::subtitles::vtt::VttSubtitle;
use crate::translation::{TranslationLimits, TranslationRequest, Translator};

#[derive(Debug)]
struct TranslationFileLock(std::fs::File);

impl Drop for TranslationFileLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

const PROGRESS_VERSION: u32 = 1;
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    /// If `true`, saves progress to a file and skips already-translated files on restart.
    pub resume: bool,
    /// Maximum number of chunks to translate concurrently. Default: 1 (sequential).
    pub max_concurrent: usize,
}

impl TranslateMkvOptions {
    /// Validate option invariants. Called by [`translate_mkv`] before any work.
    pub fn validate(&self) -> Result<()> {
        if self.target_language.trim().is_empty() {
            return Err(SubtitleToolkitError::Translation {
                provider: "options",
                message: "target_language cannot be empty".to_string(),
            });
        }
        if self.target_language != self.target_language.trim() {
            return Err(SubtitleToolkitError::Translation {
                provider: "options",
                message: "target_language cannot contain surrounding whitespace".to_string(),
            });
        }
        if self.max_concurrent == 0 {
            return Err(SubtitleToolkitError::Translation {
                provider: "options",
                message: "max_concurrent must be at least 1".to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ProgressManifest {
    version: u32,
    run: ProgressRun,
    completed: Vec<CompletedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ProgressRun {
    target_language: String,
    translator: String,
    requested_track_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CompletedFile {
    canonical_path: String,
    fingerprint: FileFingerprint,
    #[serde(default = "completed_by_default")]
    completed: bool,
}

const fn completed_by_default() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct FileFingerprint {
    size: u64,
    modified_unix_nanos: u128,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumeAction {
    Process,
    Skip,
}

fn resume_action(
    manifest: &ProgressManifest,
    canonical_path: &str,
    fingerprint: &FileFingerprint,
) -> Result<ResumeAction> {
    let Some(entry) = manifest
        .completed
        .iter()
        .find(|entry| entry.canonical_path == canonical_path)
    else {
        return Ok(ResumeAction::Process);
    };

    if entry.completed {
        return Ok(if entry.fingerprint == *fingerprint {
            ResumeAction::Skip
        } else {
            ResumeAction::Process
        });
    }

    if entry.fingerprint == *fingerprint {
        return Ok(ResumeAction::Process);
    }

    Err(SubtitleToolkitError::Progress {
        message: format!(
            "{canonical_path} changed after translation started but before its checkpoint was committed; \
                inspect the MKV before resuming"
        ),
    })
}

/// Translate supported subtitles in MKV file(s) and mux the result back in-place.
///
/// For each MKV file:
/// 1. Inspects tracks and selects ASS, SRT, WebVTT, or PGS
/// 2. Extracts the subtitle to a temporary directory
/// 3. Protects inline formatting spans
/// 4. Chunks cues according to provider item and byte limits
/// 5. Translates each chunk via the provided [`Translator`]
/// 6. Validates and restores protected formatting
/// 7. Builds and validates a replacement MKV while preserving track metadata/order
///
/// If `input` is a directory, processes all `.mkv` files sequentially.
pub async fn translate_mkv(
    options: TranslateMkvOptions,
    translator: Arc<dyn Translator>,
) -> Result<()> {
    options.validate()?;
    let files = discover_mkv_files(&options.input).await?;
    let use_resume = options.resume && !options.dry_run;
    let progress_path = progress_file_path(&options.input);
    let run = ProgressRun {
        target_language: options.target_language.trim().to_string(),
        translator: translator.identifier(),
        requested_track_id: options.track_id,
    };
    let mut manifest = if use_resume && progress_path.exists() {
        load_progress(&progress_path, &run).await?
    } else {
        ProgressManifest {
            version: PROGRESS_VERSION,
            run,
            completed: Vec::new(),
        }
    };

    let total = files.len();
    for (i, file) in files.into_iter().enumerate() {
        let canonical_path = canonical_path_string(&file).await?;
        let fingerprint = file_fingerprint(&file).await?;

        if use_resume
            && resume_action(&manifest, &canonical_path, &fingerprint)? == ResumeAction::Skip
        {
            eprintln!(
                "[resume] skipping ({}/{}): {}",
                i + 1,
                total,
                file.display()
            );
            continue;
        }
        if use_resume {
            manifest
                .completed
                .retain(|entry| entry.canonical_path != canonical_path);
            manifest.completed.push(CompletedFile {
                canonical_path: canonical_path.clone(),
                fingerprint: fingerprint.clone(),
                completed: false,
            });
            save_progress_atomic(&progress_path, &manifest).await?;
        }

        translate_one(file.clone(), &options, translator.clone()).await?;

        if use_resume {
            let completed = manifest
                .completed
                .iter_mut()
                .find(|entry| entry.canonical_path == canonical_path)
                .ok_or_else(|| SubtitleToolkitError::Progress {
                    message: format!("internal progress entry for {} disappeared", file.display()),
                })?;
            completed.fingerprint = file_fingerprint(&file).await?;
            completed.completed = true;
            save_progress_atomic(&progress_path, &manifest).await?;
            eprintln!(
                "[resume] progress saved ({}/{})",
                manifest
                    .completed
                    .iter()
                    .filter(|entry| entry.completed)
                    .count(),
                total
            );
        }
    }

    if use_resume && progress_path.exists() {
        tokio::fs::remove_file(&progress_path).await?;
    }

    Ok(())
}

async fn load_progress(path: &Path, expected_run: &ProgressRun) -> Result<ProgressManifest> {
    let data = tokio::fs::read_to_string(path).await?;
    let manifest: ProgressManifest =
        serde_json::from_str(&data).map_err(|error| SubtitleToolkitError::Progress {
            message: format!(
                "{} is not a valid progress manifest: {error}",
                path.display()
            ),
        })?;
    if manifest.version != PROGRESS_VERSION {
        return Err(SubtitleToolkitError::Progress {
            message: format!(
                "{} uses unsupported version {}; expected {}",
                path.display(),
                manifest.version,
                PROGRESS_VERSION
            ),
        });
    }
    if &manifest.run != expected_run {
        return Err(SubtitleToolkitError::Progress {
            message: format!(
                "{} belongs to a different language, provider/model, or track selection",
                path.display()
            ),
        });
    }
    Ok(manifest)
}

async fn save_progress_atomic(path: &Path, manifest: &ProgressManifest) -> Result<()> {
    let json = serde_json::to_vec_pretty(manifest)?;
    let suffix = unique_suffix();
    let temp_path = path.with_extension(format!("json.tmp-{suffix}"));
    let backup_path = path.with_extension(format!("json.bak-{suffix}"));
    tokio::fs::write(&temp_path, json).await?;

    let had_previous = path.exists();
    if had_previous {
        tokio::fs::rename(path, &backup_path).await?;
    }
    if let Err(error) = tokio::fs::rename(&temp_path, path).await {
        if had_previous {
            let _ = tokio::fs::rename(&backup_path, path).await;
        }
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(error.into());
    }
    if had_previous {
        tokio::fs::remove_file(backup_path).await?;
    }
    Ok(())
}

async fn canonical_path_string(path: &Path) -> Result<String> {
    Ok(tokio::fs::canonicalize(path)
        .await?
        .to_string_lossy()
        .into_owned())
}

async fn file_fingerprint(path: &Path) -> Result<FileFingerprint> {
    let metadata = tokio::fs::metadata(path).await?;
    let modified_unix_nanos = metadata
        .modified()?
        .duration_since(UNIX_EPOCH)
        .map_err(|error| SubtitleToolkitError::Progress {
            message: format!(
                "{} has an invalid modification time: {error}",
                path.display()
            ),
        })?
        .as_nanos();
    Ok(FileFingerprint {
        size: metadata.len(),
        modified_unix_nanos,
    })
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{nanos}-{counter}", std::process::id())
}

async fn translate_one(
    file: PathBuf,
    options: &TranslateMkvOptions,
    translator: Arc<dyn Translator>,
) -> Result<()> {
    let _file_lock = acquire_translation_file_lock(&file)?;
    let info = inspect_mkv(&file).await?;
    let (track, format) = select_subtitle_track(&info, options.track_id)
        .ok_or_else(|| SubtitleToolkitError::NoSubtitleTrack { path: file.clone() })?;

    let temp_dir = tempdir()?;

    // PGS requires special handling: extract .sup, OCR to SRT, then translate
    if format == SubtitleFormat::Pgs {
        let sup_path = temp_dir.path().join("source.sup");
        let translated_path = temp_dir.path().join("translated.srt");

        eprintln!("[translate] {} (PGS → OCR)", file.display());
        extract_subtitle(&file, track.id, &sup_path).await?;

        if options.dry_run {
            println!(
                "[dry-run] {}: PGS bitmap subtitle (requires OCR)",
                file.display()
            );
            return Ok(());
        }

        let ocr_config = OcrConfig {
            stream_language: track.properties.language.clone(),
            ..OcrConfig::default()
        };
        let srt_text = ocr_pgs_to_srt(&sup_path, &ocr_config).await?;
        let srt = SrtSubtitle::parse(&srt_text)?;
        let source_cue_count = srt.document().cues.len();

        eprintln!(
            "[translate] OCR extracted {} cues, translating",
            srt.document().cues.len()
        );
        let translated = translate_srt(
            srt,
            &options.target_language,
            options.max_concurrent,
            translator,
        )
        .await?;

        let rendered = translated.render();
        validate_rendered_subtitle(SubtitleFormat::Srt, &rendered, source_cue_count)?;
        tokio::fs::write(&translated_path, rendered).await?;
        if options.keep_temp {
            persist_temp_artifacts(
                &file,
                &[
                    ("source.sup", &sup_path),
                    ("translated.srt", &translated_path),
                ],
            )
            .await?;
        }

        eprintln!("[translate] muxing translated subtitle");
        mux_subtitle_in_place(&file, track.id, &translated_path, &options.target_language).await?;
        eprintln!("[translate] done: {}", file.display());
        return Ok(());
    }

    let ext = match format {
        SubtitleFormat::Ass => "ass",
        SubtitleFormat::Srt => "srt",
        SubtitleFormat::Vtt => "vtt",
        SubtitleFormat::Pgs => unreachable!(),
    };

    let extracted_path = temp_dir.path().join(format!("source.{ext}"));
    let translated_path = temp_dir.path().join(format!("translated.{ext}"));

    let format_label = match format {
        SubtitleFormat::Ass => "ASS",
        SubtitleFormat::Srt => "SRT",
        SubtitleFormat::Vtt => "VTT",
        SubtitleFormat::Pgs => unreachable!(),
    };
    eprintln!("[translate] {} ({})", file.display(), format_label);
    extract_subtitle(&file, track.id, &extracted_path).await?;

    let source = tokio::fs::read_to_string(&extracted_path).await?;

    let (rendered, source_cue_count) = match format {
        SubtitleFormat::Ass => {
            let ass = AssSubtitle::parse(&source)?;
            if options.dry_run {
                let summary = dry_run_summary(ass.document(), &options.target_language);
                println!("[dry-run] {}: {}", file.display(), summary);
                return Ok(());
            }
            let cue_count = ass.document().cues.len();
            let rendered = translate_ass(
                ass,
                &options.target_language,
                options.max_concurrent,
                translator,
            )
            .await?
            .render();
            (rendered, cue_count)
        }
        SubtitleFormat::Srt => {
            let srt = SrtSubtitle::parse(&source)?;
            if options.dry_run {
                let summary = dry_run_summary(srt.document(), &options.target_language);
                println!("[dry-run] {}: {}", file.display(), summary);
                return Ok(());
            }
            let cue_count = srt.document().cues.len();
            let rendered = translate_srt(
                srt,
                &options.target_language,
                options.max_concurrent,
                translator,
            )
            .await?
            .render();
            (rendered, cue_count)
        }
        SubtitleFormat::Vtt => {
            let vtt = VttSubtitle::parse(&source)?;
            if options.dry_run {
                let summary = dry_run_summary(vtt.document(), &options.target_language);
                println!("[dry-run] {}: {}", file.display(), summary);
                return Ok(());
            }
            let cue_count = vtt.document().cues.len();
            let rendered = translate_vtt(
                vtt,
                &options.target_language,
                options.max_concurrent,
                translator,
            )
            .await?
            .render();
            (rendered, cue_count)
        }
        SubtitleFormat::Pgs => unreachable!(),
    };

    validate_rendered_subtitle(format, &rendered, source_cue_count)?;
    tokio::fs::write(&translated_path, &rendered).await?;
    if options.keep_temp {
        persist_temp_artifacts(
            &file,
            &[
                (&format!("source.{ext}"), &extracted_path),
                (&format!("translated.{ext}"), &translated_path),
            ],
        )
        .await?;
    }

    eprintln!("[translate] muxing translated subtitle");
    mux_subtitle_in_place(&file, track.id, &translated_path, &options.target_language).await?;
    eprintln!("[translate] done: {}", file.display());

    Ok(())
}

fn acquire_translation_file_lock(file: &Path) -> Result<TranslationFileLock> {
    let canonical = std::fs::canonicalize(file)?;
    let identity = canonical.to_string_lossy();
    let identity = if cfg!(windows) {
        identity.to_ascii_lowercase()
    } else {
        identity.into_owned()
    };
    let digest = format!("{:x}", Sha256::digest(identity.as_bytes()));
    let lock_dir = std::env::temp_dir().join("psyche-subtitle-toolkit-locks");
    std::fs::create_dir_all(&lock_dir)?;
    let lock_path = lock_dir.join(format!("{digest}.lock"));
    let lock_file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)?;
    fs2::FileExt::try_lock_exclusive(&lock_file).map_err(|error| {
        SubtitleToolkitError::Io(std::io::Error::other(format!(
            "another translation is already processing {} (lock {}): {error}",
            file.display(),
            lock_path.display()
        )))
    })?;
    Ok(TranslationFileLock(lock_file))
}

async fn persist_temp_artifacts(file: &Path, artifacts: &[(&str, &PathBuf)]) -> Result<()> {
    let persisted = file.with_extension("psyche-subtitle-toolkit-temp");
    tokio::fs::create_dir_all(&persisted).await?;
    for (name, source) in artifacts {
        tokio::fs::copy(source, persisted.join(name)).await?;
    }
    Ok(())
}

fn validate_rendered_subtitle(
    format: SubtitleFormat,
    rendered: &str,
    expected_cues: usize,
) -> Result<()> {
    let actual_cues = match format {
        SubtitleFormat::Ass => AssSubtitle::parse(rendered)?.document().cues.len(),
        SubtitleFormat::Srt => SrtSubtitle::parse(rendered)?.document().cues.len(),
        SubtitleFormat::Vtt => VttSubtitle::parse(rendered)?.document().cues.len(),
        SubtitleFormat::Pgs => unreachable!("PGS is converted to SRT before validation"),
    };
    if expected_cues == 0 || actual_cues != expected_cues {
        return Err(SubtitleToolkitError::InvalidTranslation {
            message: format!(
                "rendered subtitle contains {actual_cues} cues; expected {expected_cues}"
            ),
        });
    }
    Ok(())
}

fn progress_file_path(input: &std::path::Path) -> PathBuf {
    if input.is_dir() {
        input.join(".psyche-subtitle-toolkit-progress.json")
    } else if input.extension().is_some() {
        let parent = input.parent().unwrap_or_else(|| Path::new("."));
        let name = input
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("input.mkv");
        parent.join(format!(".{name}.psyche-subtitle-toolkit-progress.json"))
    } else {
        input.join(".psyche-subtitle-toolkit-progress.json")
    }
}

/// Generate a dry-run summary for a subtitle document: cue count, char count, chunk count.
pub fn dry_run_summary(doc: &SubtitleDocument, target_language: &str) -> String {
    let (clean_doc, _) = strip_tags(doc);
    let limits = TranslationLimits::default();
    let chunks = chunk_document_with_limits(&clean_doc, limits.max_items, limits.max_request_bytes)
        .unwrap_or_default();
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
/// 1. Protects all ASS override tags, including inline tags
/// 2. Chunks cues according to provider item and byte limits
/// 3. Translates each chunk via the provided [`Translator`]
/// 4. Applies translated text back to the document
/// 5. Validates and restores the original override tags
///
/// Returns the translated [`AssSubtitle`]. Use [`AssSubtitle::render`] to get
/// the final ASS string.
pub async fn translate_ass(
    mut ass: AssSubtitle,
    target_language: &str,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<AssSubtitle> {
    validate_direct_translation_options(ass.document(), target_language, max_concurrent)?;
    let (mut clean_doc, tag_map) = protect_ass_spans(ass.document());
    clean_doc = translate_document(clean_doc, target_language, max_concurrent, translator).await?;
    restore_protected_spans(&mut clean_doc, &tag_map)?;
    *ass.document_mut() = clean_doc;
    Ok(ass)
}

/// Translate an SRT subtitle through the processing pipeline.
///
/// Protects inline markup, translates provider-sized chunks, then restores markup.
///
/// Returns the translated [`SrtSubtitle`]. Use [`SrtSubtitle::render`] to get
/// the final SRT string.
pub async fn translate_srt(
    mut srt: SrtSubtitle,
    target_language: &str,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<SrtSubtitle> {
    validate_direct_translation_options(srt.document(), target_language, max_concurrent)?;
    let (clean_doc, tag_map) = protect_markup_spans(srt.document());
    let mut clean_doc =
        translate_document(clean_doc, target_language, max_concurrent, translator).await?;
    restore_protected_spans(&mut clean_doc, &tag_map)?;
    *srt.document_mut() = clean_doc;
    Ok(srt)
}

/// Translate a WebVTT subtitle through the processing pipeline.
///
/// Protects inline markup, translates provider-sized chunks, then restores markup.
///
/// Returns the translated [`VttSubtitle`]. Use [`VttSubtitle::render`] to get
/// the final WebVTT string.
pub async fn translate_vtt(
    mut vtt: VttSubtitle,
    target_language: &str,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<VttSubtitle> {
    validate_direct_translation_options(vtt.document(), target_language, max_concurrent)?;
    let (clean_doc, tag_map) = protect_markup_spans(vtt.document());
    let mut clean_doc =
        translate_document(clean_doc, target_language, max_concurrent, translator).await?;
    restore_protected_spans(&mut clean_doc, &tag_map)?;
    *vtt.document_mut() = clean_doc;
    Ok(vtt)
}

fn validate_direct_translation_options(
    document: &SubtitleDocument,
    target_language: &str,
    max_concurrent: usize,
) -> Result<()> {
    if target_language.trim().is_empty() {
        return Err(SubtitleToolkitError::Translation {
            provider: "options",
            message: "target_language cannot be empty".into(),
        });
    }
    if target_language != target_language.trim() {
        return Err(SubtitleToolkitError::Translation {
            provider: "options",
            message: "target_language cannot contain surrounding whitespace".into(),
        });
    }
    if max_concurrent == 0 {
        return Err(SubtitleToolkitError::Translation {
            provider: "options",
            message: "max_concurrent must be at least 1".into(),
        });
    }
    if document.cues.is_empty() {
        return Err(SubtitleToolkitError::InvalidTranslation {
            message: "subtitle document contains no cues".into(),
        });
    }
    Ok(())
}

/// Core translation logic shared by ASS, SRT, and VTT pipelines.
async fn translate_document(
    mut doc: SubtitleDocument,
    target_language: &str,
    max_concurrent: usize,
    translator: Arc<dyn Translator>,
) -> Result<SubtitleDocument> {
    let translatable = SubtitleDocument {
        cues: doc
            .cues
            .iter()
            .filter(|cue| !cue.text.trim().is_empty())
            .cloned()
            .collect(),
    };
    if translatable.cues.is_empty() {
        return Ok(doc);
    }

    let limits = translator.limits();
    let chunks =
        chunk_document_with_limits(&translatable, limits.max_items, limits.max_request_bytes)?;
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
        let source_chunk = chunk.clone();
        let semaphore = semaphore.clone();
        let translator = translator.clone();
        let target = target_language.to_string();

        join_set.spawn(async move {
            let permit = semaphore.acquire_owned().await.map_err(|error| {
                SubtitleToolkitError::Translation {
                    provider: "pipeline",
                    message: format!("semaphore closed: {error}"),
                }
            });
            let permit = match permit {
                Ok(permit) => permit,
                Err(error) => return (i, Err(error)),
            };
            let _permit = permit;
            let numbered_clone = numbered.clone();
            let ids_clone = ids.clone();
            let source_chunk_clone = source_chunk.clone();
            let result = retry_async(3, || {
                let numbered = numbered_clone.clone();
                let ids = ids_clone.clone();
                let source_chunk = source_chunk_clone.clone();
                let translator = translator.clone();
                let target = target.clone();
                async move {
                    let translated_text = translator
                        .translate(TranslationRequest {
                            source_text: &numbered,
                            target_language: &target,
                        })
                        .await?;
                    let parsed = parse_numbered_text(&translated_text, &ids)?;
                    validate_translated_chunk(&source_chunk, &parsed, limits)?;
                    Ok(parsed)
                }
            })
            .await;
            (i, result)
        });
    }

    let mut ordered_results = BTreeMap::new();
    while let Some(result) = join_set.join_next().await {
        let (i, outcome) = result.map_err(|e| SubtitleToolkitError::Translation {
            provider: "pipeline",
            message: format!("task panicked: {e}"),
        })?;
        match outcome {
            Ok(translated) => {
                ordered_results.insert(i, translated);
            }
            Err(error) => {
                join_set.abort_all();
                while join_set.join_next().await.is_some() {}
                return Err(error);
            }
        }
    }

    let mut all_translated = BTreeMap::new();
    for translated in ordered_results.into_values() {
        all_translated.extend(translated);
    }

    apply_translation(&mut doc, all_translated);
    Ok(doc)
}

fn validate_translated_chunk(
    source: &SubtitleDocument,
    translated: &BTreeMap<usize, String>,
    limits: TranslationLimits,
) -> Result<()> {
    for cue in &source.cues {
        let source_tokens = protected_tokens(&cue.text);
        let translated_tokens = translated
            .get(&cue.id)
            .map_or_else(Vec::new, |value| protected_tokens(value));
        if source_tokens != translated_tokens {
            return Err(SubtitleToolkitError::InvalidTranslation {
                message: format!("protected formatting tokens changed for id <{}>", cue.id),
            });
        }
    }

    if !limits.reject_unchanged_output {
        return Ok(());
    }

    let alphabetic_count: usize = source
        .cues
        .iter()
        .map(|cue| {
            cue.text
                .chars()
                .filter(|character| character.is_alphabetic())
                .count()
        })
        .sum();
    let unchanged = source.cues.iter().all(|cue| {
        translated
            .get(&cue.id)
            .is_some_and(|value| value == &cue.text)
    });
    if unchanged && alphabetic_count >= 12 {
        return Err(SubtitleToolkitError::InvalidTranslation {
            message: "provider returned the source chunk unchanged".into(),
        });
    }
    Ok(())
}

fn protected_tokens(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut remaining = text;
    while let Some(start) = remaining.find("[[PSY_TAG_") {
        let candidate = &remaining[start..];
        let Some(end) = candidate.find("]]") else {
            break;
        };
        let end = end + 2;
        tokens.push(&candidate[..end]);
        remaining = &candidate[end..];
    }
    tokens
}

#[cfg(test)]
mod options_tests {
    use super::*;
    use std::path::PathBuf;

    fn opts(target_language: &str, max_concurrent: usize) -> TranslateMkvOptions {
        TranslateMkvOptions {
            input: PathBuf::from("/tmp/nonexistent.mkv"),
            target_language: target_language.to_string(),
            track_id: None,
            keep_temp: false,
            dry_run: false,
            resume: false,
            max_concurrent,
        }
    }

    #[test]
    fn validate_accepts_minimal_valid_options() {
        assert!(opts("pt-BR", 1).validate().is_ok());
        assert!(opts("en", 8).validate().is_ok());
    }

    #[test]
    fn validate_rejects_empty_target_language() {
        let error = opts("", 1).validate().unwrap_err();
        assert!(matches!(error, SubtitleToolkitError::Translation { .. }));
    }

    #[test]
    fn validate_rejects_whitespace_target_language() {
        assert!(opts("   ", 1).validate().is_err());
    }

    #[test]
    fn validate_rejects_zero_concurrency() {
        let error = opts("pt-BR", 0).validate().unwrap_err();
        assert!(matches!(error, SubtitleToolkitError::Translation { .. }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SubtitleToolkitError;
    use crate::subtitles::ass::AssSubtitle;
    use crate::subtitles::srt::SrtSubtitle;
    use crate::subtitles::vtt::VttSubtitle;
    use crate::translation::{TranslationLimits, TranslationRequest, Translator};
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
        responses.insert(
            "<1> Hello world\n<2> Goodbye world".to_string(),
            "<1> Olá mundo\n<2> Adeus mundo".to_string(),
        );

        let translator = Arc::new(FakeTranslator::new(responses));
        let result = translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();
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
        responses.insert(
            "<1> Hello world\n<2> Goodbye world".to_string(),
            "<1> translated1\n<2> translated2".to_string(),
        );

        let translator = Arc::new(FakeTranslator::new(responses));
        translate_ass(ass, "ja", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();

        let texts = translator.received_texts();
        assert_eq!(texts.len(), 1);
        assert_eq!(texts[0], "<1> Hello world\n<2> Goodbye world");
    }

    #[tokio::test]
    async fn pipeline_strips_and_reinjects_override_tags() {
        let ass = AssSubtitle::parse(ASS_WITH_TAGS).unwrap();

        // The translator receives protected tokens instead of raw ASS commands.
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> [[PSY_TAG_0]][[PSY_TAG_1]]Status line\n<2> Normal text".to_string(),
            "<1> [[PSY_TAG_0]][[PSY_TAG_1]]Linha de status\n<2> Texto normal".to_string(),
        );

        let translator = Arc::new(FakeTranslator::new(responses));
        let result = translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();
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
        assert!(texts[0].contains("[[PSY_TAG_0]][[PSY_TAG_1]]"));
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
                i,
                i + 1,
            ));
        }

        let ass_content = lines.join("\n");

        // Verify chunking splits at 200 lines
        let ass = AssSubtitle::parse(&ass_content).unwrap();
        let (clean_doc, _) = crate::subtitles::structured::strip_tags(ass.document());
        let chunks = crate::subtitles::structured::chunk_document_by_lines(&clean_doc, 200);
        assert_eq!(
            chunks.len(),
            2,
            "300 cues should produce 2 chunks at 200 lines"
        );
        assert_eq!(chunks[0].cues.len(), 200);
        assert_eq!(chunks[1].cues.len(), 100);

        // Build a FakeTranslator (identity translation)
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        let ass = AssSubtitle::parse(&ass_content).unwrap();
        let result = translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();
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

    struct TwoItemTranslator {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl Translator for TwoItemTranslator {
        async fn translate(&self, request: TranslationRequest<'_>) -> crate::error::Result<String> {
            self.calls
                .lock()
                .unwrap()
                .push(request.source_text.to_string());
            Ok(request.source_text.replace("line", "linha"))
        }

        fn limits(&self) -> TranslationLimits {
            TranslationLimits {
                max_items: 2,
                max_request_bytes: 1_024,
                reject_unchanged_output: false,
            }
        }
    }

    #[tokio::test]
    async fn pipeline_respects_provider_item_limit() {
        let mut ass = SIMPLE_ASS.to_string();
        ass.push_str(
            "Dialogue: 0,0:00:05.00,0:00:06.00,Default,,0,0,0,,third line\n\
             Dialogue: 0,0:00:07.00,0:00:08.00,Default,,0,0,0,,fourth line\n\
             Dialogue: 0,0:00:09.00,0:00:10.00,Default,,0,0,0,,fifth line\n",
        );
        let translator = Arc::new(TwoItemTranslator {
            calls: Mutex::new(Vec::new()),
        });
        translate_ass(
            AssSubtitle::parse(&ass).unwrap(),
            "pt-BR",
            1,
            translator.clone(),
        )
        .await
        .unwrap();

        let calls = translator.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert!(calls.iter().all(|call| call.lines().count() <= 2));
    }

    #[test]
    fn strict_validation_rejects_nontrivial_identity_output() {
        let source = SubtitleDocument {
            cues: vec![crate::SubtitleCue {
                id: 1,
                text: "This is unchanged dialogue".into(),
            }],
        };
        let translated = BTreeMap::from([(1, "This is unchanged dialogue".into())]);
        let error = validate_translated_chunk(
            &source,
            &translated,
            TranslationLimits {
                reject_unchanged_output: true,
                ..TranslationLimits::default()
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("unchanged"));
    }

    #[test]
    fn chunk_validation_rejects_changed_protected_tokens() {
        let source = SubtitleDocument {
            cues: vec![crate::SubtitleCue {
                id: 1,
                text: "[[PSY_TAG_0]]Hello[[PSY_TAG_1]]".into(),
            }],
        };
        let translated = BTreeMap::from([(1, "[[PSY_TAG_0]]OlÃ¡".into())]);
        let error = validate_translated_chunk(&source, &translated, TranslationLimits::default())
            .unwrap_err();
        assert!(error.to_string().contains("formatting tokens changed"));
    }

    #[tokio::test]
    async fn pipeline_propagates_translator_error() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let translator = FakeTranslator::with_error("rate limit exceeded");

        let err = translate_ass(ass, "pt-BR", 1, Arc::new(translator) as Arc<dyn Translator>)
            .await
            .unwrap_err();

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
        let err = translate_ass(ass, "pt-BR", 1, Arc::new(translator) as Arc<dyn Translator>)
            .await
            .unwrap_err();

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
        let result = translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();
        let rendered = result.render();

        assert!(rendered.contains("Primeira linha"));
        assert!(rendered.contains("Segunda linha"));
        assert!(rendered.contains(r"Primeira linha\NSegunda linha"));
    }

    #[tokio::test]
    async fn srt_pipeline_restores_real_multiline_and_markup() {
        let srt =
            SrtSubtitle::parse("1\r\n00:00:01,000 --> 00:00:02,000\r\n<i>Hello</i>\r\nworld\r\n")
                .unwrap();
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> [[PSY_TAG_0]]Hello[[PSY_TAG_1]]\\Nworld".into(),
            "<1> [[PSY_TAG_0]]OlÃ¡[[PSY_TAG_1]]\\Nmundo".into(),
        );
        let translated = translate_srt(srt, "pt-BR", 1, Arc::new(FakeTranslator::new(responses)))
            .await
            .unwrap();
        let rendered = translated.render();
        assert!(rendered.contains("<i>OlÃ¡</i>\nmundo"));
        assert!(!rendered.contains(r"\N"));
    }

    #[tokio::test]
    async fn vtt_pipeline_restores_real_multiline_and_markup() {
        let vtt = VttSubtitle::parse(
            "WEBVTT\r\n\r\n1\r\n00:00:01.000 --> 00:00:02.000\r\n<v Speaker>Hello</v>\r\nworld\r\n",
        )
        .unwrap();
        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> [[PSY_TAG_0]]Hello[[PSY_TAG_1]]\\Nworld".into(),
            "<1> [[PSY_TAG_0]]OlÃ¡[[PSY_TAG_1]]\\Nmundo".into(),
        );
        let translated = translate_vtt(vtt, "pt-BR", 1, Arc::new(FakeTranslator::new(responses)))
            .await
            .unwrap();
        let rendered = translated.render();
        assert!(rendered.contains("<v Speaker>OlÃ¡</v>\nmundo"));
        assert!(!rendered.contains(r"\N"));
    }

    #[tokio::test]
    async fn direct_pipeline_rejects_zero_concurrency_without_hanging() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            translate_ass(
                ass,
                "pt-BR",
                0,
                Arc::new(FakeTranslator::new(std::collections::HashMap::new())),
            ),
        )
        .await
        .expect("validation must return without waiting");
        assert!(result.unwrap_err().to_string().contains("max_concurrent"));
    }

    #[tokio::test]
    async fn pipeline_translates_basic_document() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();

        let mut responses = std::collections::HashMap::new();
        responses.insert(
            "<1> Hello world\n<2> Goodbye world".to_string(),
            "<1> translated1\n<2> translated2".to_string(),
        );

        let translator = Arc::new(FakeTranslator::new(responses));
        translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();

        let texts = translator.received_texts();
        assert_eq!(texts.len(), 1);
    }

    #[test]
    fn dry_run_summary_reports_cues_chars_chunks() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let summary = dry_run_summary(ass.document(), "pt-BR");

        assert!(
            summary.contains("2 cues"),
            "expected '2 cues' in: {summary}"
        );
        assert!(
            summary.contains("1 chunk(s)"),
            "expected '1 chunk(s)' in: {summary}"
        );
        assert!(
            summary.contains("→ pt-BR"),
            "expected '→ pt-BR' in: {summary}"
        );
    }

    #[test]
    fn dry_run_summary_counts_chars() {
        let ass = AssSubtitle::parse(SIMPLE_ASS).unwrap();
        let summary = dry_run_summary(ass.document(), "en");

        // "Hello world" = 11 chars, "Goodbye world" = 13 chars = 24 total
        assert!(
            summary.contains("24 chars"),
            "expected '24 chars' in: {summary}"
        );
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

        assert!(
            summary.contains("0 cues"),
            "expected '0 cues' in: {summary}"
        );
        assert!(
            summary.contains("0 chars"),
            "expected '0 chars' in: {summary}"
        );
        assert!(
            summary.contains("0 chunk(s)"),
            "expected '0 chunk(s)' in: {summary}"
        );
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
                "Dialogue: 0,0:00:{:02}.00,0:00:{:02}.00,Default,,0,0,0,,\
                    This is subtitle line number {i} with enough text to fill space",
                i, i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let summary = dry_run_summary(ass.document(), "pt-BR");

        assert!(
            summary.contains("300 cues"),
            "expected '300 cues' in: {summary}"
        );
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
            "<1> Olá mundo".to_string(),                  // missing <2>
            "<1> Olá mundo\n<2> Adeus mundo".to_string(), // correct
        ]));

        let result = translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap();
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

        let err = translate_ass(ass, "pt-BR", 1, translator.clone() as Arc<dyn Translator>)
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
            std::path::PathBuf::from(
                "/media/anime/.episode.mkv.psyche-subtitle-toolkit-progress.json"
            )
        );
    }

    #[tokio::test]
    async fn progress_file_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");
        let run = ProgressRun {
            target_language: "pt-BR".into(),
            translator: "fake:model".into(),
            requested_track_id: Some(2),
        };
        let manifest = ProgressManifest {
            version: PROGRESS_VERSION,
            run: run.clone(),
            completed: vec![CompletedFile {
                canonical_path: "/media/anime/ep1.mkv".into(),
                fingerprint: FileFingerprint {
                    size: 42,
                    modified_unix_nanos: 123,
                },
                completed: true,
            }],
        };

        save_progress_atomic(&progress_path, &manifest)
            .await
            .unwrap();
        let loaded = load_progress(&progress_path, &run).await.unwrap();
        assert_eq!(loaded, manifest);
    }

    #[test]
    fn resume_retries_an_unchanged_pending_file() {
        let fingerprint = FileFingerprint {
            size: 42,
            modified_unix_nanos: 123,
        };
        let manifest = ProgressManifest {
            version: PROGRESS_VERSION,
            run: ProgressRun {
                target_language: "pt-BR".into(),
                translator: "fake:model".into(),
                requested_track_id: None,
            },
            completed: vec![CompletedFile {
                canonical_path: "/media/anime/ep1.mkv".into(),
                fingerprint: fingerprint.clone(),
                completed: false,
            }],
        };

        assert_eq!(
            resume_action(&manifest, "/media/anime/ep1.mkv", &fingerprint).unwrap(),
            ResumeAction::Process
        );
    }

    #[test]
    fn resume_rejects_a_changed_pending_file() {
        let manifest = ProgressManifest {
            version: PROGRESS_VERSION,
            run: ProgressRun {
                target_language: "pt-BR".into(),
                translator: "fake:model".into(),
                requested_track_id: None,
            },
            completed: vec![CompletedFile {
                canonical_path: "/media/anime/ep1.mkv".into(),
                fingerprint: FileFingerprint {
                    size: 42,
                    modified_unix_nanos: 123,
                },
                completed: false,
            }],
        };
        let changed = FileFingerprint {
            size: 84,
            modified_unix_nanos: 456,
        };

        let error = resume_action(&manifest, "/media/anime/ep1.mkv", &changed).unwrap_err();
        assert!(error.to_string().contains("inspect the MKV"));
    }

    #[tokio::test]
    async fn progress_file_handles_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");

        assert!(!progress_path.exists());
    }

    #[tokio::test]
    async fn progress_file_handles_corrupted_json() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");

        tokio::fs::write(&progress_path, "not valid json")
            .await
            .unwrap();

        let run = ProgressRun {
            target_language: "pt-BR".into(),
            translator: "fake:model".into(),
            requested_track_id: None,
        };
        let error = load_progress(&progress_path, &run).await.unwrap_err();
        assert!(matches!(error, SubtitleToolkitError::Progress { .. }));
    }

    #[tokio::test]
    async fn progress_file_rejects_different_run_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let progress_path = dir.path().join(".psyche-subtitle-toolkit-progress.json");
        let original_run = ProgressRun {
            target_language: "pt-BR".into(),
            translator: "openai:model-a".into(),
            requested_track_id: Some(2),
        };
        let manifest = ProgressManifest {
            version: PROGRESS_VERSION,
            run: original_run,
            completed: Vec::new(),
        };
        save_progress_atomic(&progress_path, &manifest)
            .await
            .unwrap();

        let different_run = ProgressRun {
            target_language: "es".into(),
            translator: "openai:model-b".into(),
            requested_track_id: Some(3),
        };
        let error = load_progress(&progress_path, &different_run)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("different"));
    }

    #[test]
    fn translation_file_lock_rejects_concurrent_writer() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("episode.mkv");
        std::fs::write(&file, b"fixture").unwrap();
        let first = acquire_translation_file_lock(&file).unwrap();
        let error = acquire_translation_file_lock(&file).unwrap_err();
        assert!(error.to_string().contains("already processing"));
        drop(first);
        acquire_translation_file_lock(&file).unwrap();
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
                i,
                i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        // Translate with max_concurrent=2 (both chunks in parallel)
        let result = translate_ass(ass, "pt-BR", 2, translator.clone() as Arc<dyn Translator>)
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
                i,
                i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        let result = translate_ass(ass, "pt-BR", 5, translator.clone() as Arc<dyn Translator>)
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
        assert_eq!(
            texts.len(),
            5,
            "expected 5 chunk calls, got {}",
            texts.len()
        );
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
                    i,
                    i + 1,
                ));
            }
            AssSubtitle::parse(&lines.join("\n")).unwrap()
        };

        let t1 = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));
        let r1 = translate_ass(make_ass(), "pt-BR", 3, t1.clone() as Arc<dyn Translator>)
            .await
            .unwrap();

        let t2 = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));
        let r2 = translate_ass(make_ass(), "pt-BR", 3, t2.clone() as Arc<dyn Translator>)
            .await
            .unwrap();

        assert_eq!(
            r1.render(),
            r2.render(),
            "concurrent output is non-deterministic"
        );
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
                i,
                i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();

        // Always error — all chunks will fail
        struct MixedResultTranslator;

        #[async_trait::async_trait]
        impl Translator for MixedResultTranslator {
            async fn translate(
                &self,
                request: TranslationRequest<'_>,
            ) -> crate::error::Result<String> {
                if request.source_text.starts_with("<201>") {
                    Err(SubtitleToolkitError::Translation {
                        provider: "mixed",
                        message: "provider down on second chunk".into(),
                    })
                } else {
                    Ok(request.source_text.to_string())
                }
            }
        }
        let translator = Arc::new(MixedResultTranslator);

        let err = translate_ass(ass, "pt-BR", 3, translator.clone() as Arc<dyn Translator>)
            .await
            .unwrap_err();

        assert!(err.to_string().contains("second chunk"));
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
            self.received
                .lock()
                .unwrap()
                .push(request.source_text.to_string());

            let current = self
                .active
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                + 1;
            // Update max_observed
            self.max_observed
                .fetch_max(current, std::sync::atomic::Ordering::SeqCst);

            // Simulate work — yield to let other tasks run
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;

            self.active
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);

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
                i,
                i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(ConcurrencyTrackingTranslator::new());

        let result = translate_ass(
            ass,
            "pt-BR",
            2, // max_concurrent=2
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        // All 600 cues should be present
        let rendered = result.render();
        for i in 1..=600 {
            assert!(
                rendered.contains(&format!("sem line {i}")),
                "missing cue {i}"
            );
        }

        // The max observed concurrency should be <= 2
        let max = translator.max_concurrent_calls();
        assert_eq!(max, 2, "expected two calls to overlap, observed {max}");

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
                i,
                i + 1,
            ));
        }
        let ass = AssSubtitle::parse(&lines.join("\n")).unwrap();
        let translator = Arc::new(FakeTranslator::new(std::collections::HashMap::new()));

        translate_ass(
            ass,
            "pt-BR",
            1, // sequential
            translator.clone() as Arc<dyn Translator>,
        )
        .await
        .unwrap();

        let texts = translator.received_texts();
        assert_eq!(texts.len(), 3);

        // In sequential mode, chunk 1 should be called before chunk 2, etc.
        // Verify by checking that each text starts with the expected cue ID range.
        assert!(
            texts[0].starts_with("<1> "),
            "first chunk should start with <1>"
        );
        assert!(
            texts[1].starts_with("<201> "),
            "second chunk should start with <201>"
        );
        assert!(
            texts[2].starts_with("<401> "),
            "third chunk should start with <401>"
        );
    }
}
