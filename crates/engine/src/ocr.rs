//! PGS (Presentation Graphic Stream) OCR pipeline.
//!
//! Extracts text from PGS bitmap subtitles using PaddleOCR PP-OCRv5.
//! Converts `.sup` files to SRT format for translation.
//!
//! Ported from [shinkai-translator](https://github.com/enrell/shinkai-translator).

use std::cmp::{Ordering, max, min};
use std::collections::HashMap;
use std::env;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::Duration;

use image::imageops::{FilterType, crop_imm, resize};
use image::{DynamicImage, Rgb, RgbImage, RgbaImage};
use ocr_rs::{OcrEngine, TextBox};
use pgs_rs::parse::{
    CompositionObject, CompositionState, ObjectDefinition, PaletteDefinition, Pgs, SegmentContents,
};
use pgs_rs::parse_pgs;
use sha2::{Digest, Sha256};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::error::{Result, SubtitleToolkitError};

const OCR_MODEL_DIR_ENV_VAR: &str = "PSYCHE_OCR_MODEL_DIR";
const OCR_MODEL_VERSION: &str = "paddleocr-v5";
const DEFAULT_LAST_CUE_DURATION_PTS: u32 = 180_000;
const MAX_CONTINUATION_GAP_PTS: u32 = 45_000;
const PTS_PER_MILLISECOND: u64 = 90;
const MIN_ALPHA: u8 = 8;
const MAX_PGS_PIXELS: usize = 33_177_600;
const MAX_PGS_FILE_BYTES: u64 = 512 * 1024 * 1024;
const OCR_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);
static DOWNLOAD_COUNTER: AtomicU64 = AtomicU64::new(0);

/// OCR language profile for PaddleOCR models.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OcrLanguage {
    /// Auto-detect based on stream language metadata.
    Auto,
    /// English recognition model.
    English,
    /// Latin script recognition model (covers most European languages).
    Latin,
}

impl OcrLanguage {
    /// Parse a language name string into an [`OcrLanguage`].
    pub fn parse_name(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "english" | "en" => Some(Self::English),
            "latin" => Some(Self::Latin),
            _ => None,
        }
    }
}

/// Configuration for PGS OCR processing.
#[derive(Clone, Debug)]
pub struct OcrConfig {
    /// OCR language profile.
    pub language: OcrLanguage,
    /// Custom model cache directory. If `None`, uses `~/.cache/psyche-subtitle-toolkit/`
    /// (name kept from the toolkit so existing model caches are reused).
    pub model_cache_dir: Option<PathBuf>,
    /// Language metadata from the selected MKV stream (for example `eng`).
    pub stream_language: Option<String>,
    /// Optional human-readable source language hint.
    pub source_language_hint: Option<String>,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            language: OcrLanguage::Auto,
            model_cache_dir: env::var(OCR_MODEL_DIR_ENV_VAR).ok().map(PathBuf::from),
            stream_language: None,
            source_language_hint: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ResolvedOcrProfile {
    English,
    Latin,
}

#[derive(Clone, Debug)]
struct ModelArtifact {
    file_name: &'static str,
    url: &'static str,
    size: u64,
    sha256: &'static str,
}

#[derive(Clone, Debug)]
struct ModelBundle {
    detection: ModelArtifact,
    recognition: ModelArtifact,
    charset: ModelArtifact,
}

#[derive(Clone, Debug)]
struct ModelPaths {
    detection: PathBuf,
    recognition: PathBuf,
    charset: PathBuf,
}

#[derive(Clone, Debug)]
struct RecognizedTextBlock {
    bbox: TextBox,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OcrCue {
    start_pts: u32,
    end_pts: u32,
    text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DisplayTextEvent {
    pts: u32,
    is_empty: bool,
    text: Option<String>,
}

#[derive(Clone, Debug)]
struct PendingCue {
    start_pts: u32,
    last_pts: u32,
    normalized_text: String,
    rendered_text: String,
}

#[derive(Debug)]
struct ResolvedDisplaySet<'a> {
    presentation_timestamp: u32,
    width: u16,
    height: u16,
    palette_id: u8,
    composition_objects: &'a [CompositionObject],
    palettes: HashMap<u8, &'a PaletteDefinition>,
    objects: HashMap<u16, &'a ObjectDefinition>,
}

impl ResolvedDisplaySet<'_> {
    fn is_empty(&self) -> bool {
        self.composition_objects.is_empty()
    }
}

/// OCR a PGS `.sup` file and return the result as SRT text.
///
/// This is the main entry point for PGS OCR. It:
/// 1. Parses the PGS binary stream
/// 2. Groups segments into display sets
/// 3. Renders each display set to an RGBA bitmap
/// 4. Runs PaddleOCR detection + recognition
/// 5. Groups text blocks into lines
/// 6. Merges consecutive display sets into timed SRT cues
///
/// Models are auto-downloaded on first run to the cache directory.
pub async fn ocr_pgs_to_srt(sup_path: &Path, config: &OcrConfig) -> Result<String> {
    let profile = resolve_profile(
        config.language,
        config.stream_language.as_deref(),
        config.source_language_hint.as_deref(),
    );
    let model_paths = ensure_models(config, profile).await?;
    let sup_path = sup_path.to_path_buf();

    tokio::task::spawn_blocking(move || ocr_pgs_to_srt_blocking(&sup_path, &model_paths))
        .await
        .map_err(|error| SubtitleToolkitError::Ocr {
            message: format!("PGS OCR task failed: {error}"),
        })?
}

/// Initialize an [`OcrEngine`]. Native runtime diagnostics are left untouched:
/// redirecting process-global stderr is not thread-safe for a library.
fn new_ocr_engine_quiet(detection: &Path, recognition: &Path, charset: &Path) -> Result<OcrEngine> {
    OcrEngine::new(detection, recognition, charset, None).map_err(|error| {
        SubtitleToolkitError::Ocr {
            message: format!("failed to initialize PaddleOCR engine: {error}"),
        }
    })
}

fn ocr_pgs_to_srt_blocking(sup_path: &Path, model_paths: &ModelPaths) -> Result<String> {
    let file_size = std::fs::metadata(sup_path)?.len();
    if file_size > MAX_PGS_FILE_BYTES {
        return Err(SubtitleToolkitError::Ocr {
            message: format!(
                "PGS stream is {file_size} bytes; maximum supported size is {MAX_PGS_FILE_BYTES}"
            ),
        });
    }
    let mut data = std::fs::read(sup_path)?;
    let pgs = parse_pgs(data.as_mut_slice()).map_err(|error| SubtitleToolkitError::Ocr {
        message: format!("failed to parse PGS subtitle stream: {error}"),
    })?;
    let display_sets = collect_display_sets(&pgs)?;
    let show_progress = std::io::stderr().is_terminal() && display_sets.len() >= 25;

    let ocr = new_ocr_engine_quiet(
        &model_paths.detection,
        &model_paths.recognition,
        &model_paths.charset,
    )?;

    let mut events = Vec::with_capacity(display_sets.len());
    if show_progress {
        eprintln!("OCR: processing {} PGS display sets", display_sets.len());
    }
    for (index, display_set) in display_sets.iter().enumerate() {
        let text = recognize_display_set_text(display_set, &ocr)?;
        events.push(DisplayTextEvent {
            pts: display_set.presentation_timestamp,
            is_empty: display_set.is_empty(),
            text,
        });
        if show_progress && ((index + 1) % 25 == 0 || index + 1 == display_sets.len()) {
            eprintln!(
                "OCR: processed {}/{} display sets",
                index + 1,
                display_sets.len()
            );
        }
    }

    let cues = merge_display_texts(&events);
    if cues.is_empty() {
        return Err(SubtitleToolkitError::Ocr {
            message: "OCR did not extract any subtitle text from the PGS stream".to_string(),
        });
    }

    Ok(render_cues_to_srt(&cues))
}

fn recognize_display_set_text(
    display_set: &ResolvedDisplaySet<'_>,
    ocr: &OcrEngine,
) -> Result<Option<String>> {
    if display_set.is_empty() {
        return Ok(None);
    }

    let image = render_display_set_for_ocr(display_set)?;

    let Some(bounds) = alpha_bounds(&image) else {
        return Ok(None);
    };

    let cropped = crop_imm(&image, bounds.0, bounds.1, bounds.2, bounds.3).to_image();
    let prepared = prepare_ocr_image(&cropped);
    let prepared_image = DynamicImage::ImageRgb8(prepared);
    let detections = ocr
        .det_model()
        .detect_and_crop(&prepared_image)
        .map_err(|error| SubtitleToolkitError::Ocr {
            message: format!("PaddleOCR detection failed on rendered PGS frame: {error}"),
        })?;

    if detections.is_empty() {
        return Ok(None);
    }

    let cropped_images = detections
        .iter()
        .map(|(image, _)| image.clone())
        .collect::<Vec<_>>();
    let recognition_results =
        ocr.recognize_batch(&cropped_images)
            .map_err(|error| SubtitleToolkitError::Ocr {
                message: format!("PaddleOCR recognition failed on rendered PGS frame: {error}"),
            })?;

    let recognized_blocks = detections
        .into_iter()
        .zip(recognition_results)
        .filter_map(|((_, bbox), recognition)| {
            let text = normalize_text_line(&recognition.text);
            if text.is_empty() {
                None
            } else {
                Some(RecognizedTextBlock { bbox, text })
            }
        })
        .collect::<Vec<_>>();

    let lines = collect_recognized_text_lines(recognized_blocks);
    let text = repair_fragmentary_ocr_lines(lines).join("\n");

    if text.trim().is_empty() {
        Ok(None)
    } else {
        Ok(Some(text))
    }
}

fn collect_display_sets<'a>(pgs: &'a Pgs) -> Result<Vec<ResolvedDisplaySet<'a>>> {
    let mut display_sets = Vec::new();
    let mut palettes = HashMap::new();
    let mut objects = HashMap::new();
    let mut index = 0usize;

    while index < pgs.segments.len() {
        let segment = &pgs.segments[index];
        let SegmentContents::PresentationComposition(presentation_composition) = &segment.contents
        else {
            index += 1;
            continue;
        };

        if presentation_composition.composition_state == CompositionState::EpochStart {
            palettes.clear();
            objects.clear();
        }

        let presentation_timestamp = segment.pts;
        let decoding_timestamp = segment.dts;
        let mut display_set = ResolvedDisplaySet {
            presentation_timestamp,
            width: presentation_composition.width,
            height: presentation_composition.height,
            palette_id: presentation_composition.palette_id,
            composition_objects: &presentation_composition.composition_objects,
            palettes: palettes.clone(),
            objects: objects.clone(),
        };
        index += 1;

        let mut saw_end = false;
        while index < pgs.segments.len() {
            let segment = &pgs.segments[index];
            if segment.pts != presentation_timestamp || segment.dts != decoding_timestamp {
                break;
            }

            match &segment.contents {
                SegmentContents::PresentationComposition(_) => {
                    return Err(SubtitleToolkitError::Ocr {
                        message: format!(
                            "unexpected nested PGS presentation composition at {presentation_timestamp}"
                        ),
                    });
                }
                SegmentContents::WindowDefinition(_) => {}
                SegmentContents::PaletteDefinition(palette_definition) => {
                    palettes.insert(palette_definition.id, palette_definition);
                    display_set
                        .palettes
                        .insert(palette_definition.id, palette_definition);
                }
                SegmentContents::ObjectDefinition(object_definition) => {
                    objects.insert(object_definition.id, object_definition);
                    display_set
                        .objects
                        .insert(object_definition.id, object_definition);
                }
                SegmentContents::End => {
                    saw_end = true;
                    index += 1;
                    break;
                }
            }

            index += 1;
        }

        if !saw_end {
            return Err(SubtitleToolkitError::Ocr {
                message: format!("unterminated PGS display set at {presentation_timestamp}"),
            });
        }

        display_sets.push(display_set);
    }

    Ok(display_sets)
}

fn render_display_set_for_ocr(display_set: &ResolvedDisplaySet<'_>) -> Result<RgbaImage> {
    let width = display_set.width as usize;
    let height = display_set.height as usize;
    let pixels = width
        .checked_mul(height)
        .filter(|pixels| *pixels > 0 && *pixels <= MAX_PGS_PIXELS)
        .ok_or_else(|| SubtitleToolkitError::Ocr {
            message: format!(
                "invalid or oversized PGS frame dimensions: {}x{}",
                display_set.width, display_set.height
            ),
        })?;
    let rgba_len = pixels
        .checked_mul(4)
        .ok_or_else(|| SubtitleToolkitError::Ocr {
            message: "PGS RGBA frame size overflow".into(),
        })?;
    let mut rgba = vec![0u8; rgba_len];

    for composition_object in display_set.composition_objects {
        let object = display_set
            .objects
            .get(&composition_object.id)
            .ok_or_else(|| SubtitleToolkitError::Ocr {
                message: format!(
                    "PGS object {} is missing from display set at {}",
                    composition_object.id, display_set.presentation_timestamp
                ),
            })?;
        let palette = display_set.palettes.get(&display_set.palette_id);
        let object_width = object.width as usize;
        let object_height = object.height as usize;
        let object_pixels = object_width
            .checked_mul(object_height)
            .filter(|pixels| *pixels > 0)
            .ok_or_else(|| SubtitleToolkitError::Ocr {
                message: format!(
                    "PGS object {} has invalid dimensions",
                    composition_object.id
                ),
            })?;
        let mut object_pixel_index = 0usize;

        for pixel in &object.data.0 {
            let palette_entry = palette.and_then(|palette| palette.entries.get(&pixel.color));
            let run_end = object_pixel_index
                .checked_add(pixel.count as usize)
                .filter(|end| *end <= object_pixels)
                .ok_or_else(|| SubtitleToolkitError::Ocr {
                    message: format!(
                        "PGS object {} run-length data exceeds {}x{} dimensions",
                        composition_object.id, object.width, object.height
                    ),
                })?;

            while object_pixel_index < run_end {
                let local_x = object_pixel_index % object_width;
                let local_y = object_pixel_index / object_width;
                let frame_x = composition_object.horizontal_position as usize + local_x;
                let frame_y = composition_object.vertical_position as usize + local_y;
                if frame_x < width && frame_y < height {
                    let frame_pixel = frame_y * width + frame_x;
                    if !is_cropped(frame_pixel, width, composition_object)
                        && let Some(entry) = palette_entry
                    {
                        let offset = frame_pixel * 4;
                        rgba[offset] = entry.luminance;
                        rgba[offset + 1] = entry.luminance;
                        rgba[offset + 2] = entry.luminance;
                        rgba[offset + 3] = entry.alpha;
                    }
                }
                object_pixel_index += 1;
            }
        }
    }

    RgbaImage::from_raw(display_set.width as u32, display_set.height as u32, rgba).ok_or_else(
        || SubtitleToolkitError::Ocr {
            message: "failed to build RGBA frame from rendered PGS subtitle".to_string(),
        },
    )
}

fn is_cropped(pixel_index: usize, frame_width: usize, object: &CompositionObject) -> bool {
    let Some(cropped) = &object.cropped else {
        return false;
    };

    let x = pixel_index % frame_width;
    let y = pixel_index / frame_width;
    let crop_left = object.horizontal_position as usize + cropped.horizontal_position as usize;
    let crop_top = object.vertical_position as usize + cropped.vertical_position as usize;
    let crop_right = crop_left + cropped.width as usize;
    let crop_bottom = crop_top + cropped.height as usize;

    x < crop_left || x >= crop_right || y < crop_top || y >= crop_bottom
}

fn alpha_bounds(image: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut found = false;

    for (x, y, pixel) in image.enumerate_pixels() {
        if pixel[3] <= MIN_ALPHA {
            continue;
        }

        found = true;
        min_x = min(min_x, x);
        min_y = min(min_y, y);
        max_x = max(max_x, x);
        max_y = max(max_y, y);
    }

    if !found {
        return None;
    }

    let padding = 6u32;
    let x = min_x.saturating_sub(padding);
    let y = min_y.saturating_sub(padding);
    let width = min(
        image.width(),
        max_x.saturating_add(padding).saturating_add(1),
    ) - x;
    let height = min(
        image.height(),
        max_y.saturating_add(padding).saturating_add(1),
    ) - y;

    Some((x, y, width.max(1), height.max(1)))
}

fn prepare_ocr_image(image: &RgbaImage) -> RgbImage {
    let mut rgb = RgbImage::new(image.width(), image.height());

    for (x, y, pixel) in image.enumerate_pixels() {
        let alpha = pixel[3] as u16;
        let red = ((pixel[0] as u16) * alpha / 255) as u8;
        let green = ((pixel[1] as u16) * alpha / 255) as u8;
        let blue = ((pixel[2] as u16) * alpha / 255) as u8;
        rgb.put_pixel(x, y, Rgb([red, green, blue]));
    }

    let scale_factor = if rgb.height() < 72 {
        3
    } else if rgb.height() < 160 {
        2
    } else {
        1
    };

    if scale_factor == 1 {
        rgb
    } else {
        resize(
            &rgb,
            rgb.width() * scale_factor,
            rgb.height() * scale_factor,
            FilterType::CatmullRom,
        )
    }
}

fn normalize_text_line(line: &str) -> String {
    line.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_for_compare(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn collect_recognized_text_lines(mut blocks: Vec<RecognizedTextBlock>) -> Vec<String> {
    if blocks.is_empty() {
        return Vec::new();
    }

    blocks.sort_by(compare_recognized_text_blocks);

    let mut lines: Vec<Vec<RecognizedTextBlock>> = Vec::with_capacity(blocks.len());
    for block in blocks {
        if let Some(current_line) = lines.last_mut()
            && belongs_to_same_line(current_line, &block)
        {
            current_line.push(block);
            continue;
        }

        lines.push(vec![block]);
    }

    lines
        .into_iter()
        .map(|mut line| {
            line.sort_by_key(|a| a.bbox.rect.left());
            render_recognized_text_line(&line)
        })
        .filter(|line| !line.is_empty())
        .collect()
}

fn compare_recognized_text_blocks(
    left: &RecognizedTextBlock,
    right: &RecognizedTextBlock,
) -> Ordering {
    left.bbox
        .rect
        .top()
        .cmp(&right.bbox.rect.top())
        .then(left.bbox.rect.left().cmp(&right.bbox.rect.left()))
        .then(left.bbox.rect.width().cmp(&right.bbox.rect.width()))
        .then(left.bbox.rect.height().cmp(&right.bbox.rect.height()))
}

fn belongs_to_same_line(
    current_line: &[RecognizedTextBlock],
    next_block: &RecognizedTextBlock,
) -> bool {
    let current_center = line_center_y(current_line);
    let next_center = text_box_center_y(&next_block.bbox);
    let current_height = line_height(current_line);
    let next_height = next_block.bbox.rect.height() as i32;
    let threshold = max(12, max(current_height, next_height) / 2);

    (next_center - current_center).abs() <= threshold
}

fn line_center_y(line: &[RecognizedTextBlock]) -> i32 {
    let (top, bottom) = line_bounds(line);
    top + ((bottom - top) / 2)
}

fn line_height(line: &[RecognizedTextBlock]) -> i32 {
    let (top, bottom) = line_bounds(line);
    (bottom - top).max(0)
}

fn line_bounds(line: &[RecognizedTextBlock]) -> (i32, i32) {
    let top = line
        .iter()
        .map(|block| block.bbox.rect.top())
        .min()
        .unwrap_or_default();
    let bottom = line
        .iter()
        .map(|block| block.bbox.rect.top() + block.bbox.rect.height() as i32)
        .max()
        .unwrap_or_default();

    (top, bottom)
}

fn text_box_center_y(bbox: &TextBox) -> i32 {
    bbox.rect.top() + (bbox.rect.height() as i32 / 2)
}

fn render_recognized_text_line(line: &[RecognizedTextBlock]) -> String {
    let mut rendered = String::new();

    for block in line {
        let piece = block.text.trim();
        if piece.is_empty() {
            continue;
        }

        append_ocr_text_piece(&mut rendered, piece);
    }

    normalize_text_line(&rendered)
}

fn append_ocr_text_piece(rendered: &mut String, piece: &str) {
    if rendered.is_empty() {
        rendered.push_str(piece);
        return;
    }

    if is_punctuation_fragment(piece) || ends_with_connector(rendered) {
        rendered.push_str(piece);
        return;
    }

    if piece.chars().next().is_some_and(|character| {
        matches!(
            character,
            ',' | '.' | '!' | '?' | ':' | ';' | ')' | ']' | '}' | '\'' | '"'
        )
    }) {
        rendered.push_str(piece);
        return;
    }

    rendered.push(' ');
    rendered.push_str(piece);
}

fn ends_with_connector(text: &str) -> bool {
    text.chars()
        .last()
        .is_some_and(|character| matches!(character, '-' | '\u{2019}' | '\'' | '/' | '('))
}

fn repair_fragmentary_ocr_lines(lines: Vec<String>) -> Vec<String> {
    let has_multiple_lines = lines.len() > 1;
    let mut repaired: Vec<String> = Vec::with_capacity(lines.len());

    for line in lines {
        let line = strip_leading_fragment_punctuation(&line);

        if is_punctuation_fragment(&line) {
            if let Some(previous) = repaired.last_mut() {
                let fragment = line.trim();
                if !previous.trim_end().ends_with(fragment) {
                    previous.push_str(fragment);
                }
            }
            continue;
        }

        if has_multiple_lines && is_noise_stub_line(&line) {
            continue;
        }

        repaired.push(line);
    }

    repaired
}

fn strip_leading_fragment_punctuation(line: &str) -> String {
    let trimmed = line.trim();
    let mut characters = trimmed.chars();
    let Some(first) = characters.next() else {
        return String::new();
    };

    if first.is_alphanumeric() || first.is_whitespace() {
        return trimmed.to_owned();
    }

    let remainder = characters.as_str().trim_start();
    if remainder.is_empty() {
        return trimmed.to_owned();
    }

    if remainder.starts_with(first) {
        return trimmed.to_owned();
    }

    if remainder
        .chars()
        .next()
        .is_some_and(|character| character.is_alphabetic())
    {
        remainder.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn is_punctuation_fragment(line: &str) -> bool {
    let trimmed = line.trim();
    !trimmed.is_empty()
        && trimmed.chars().count() <= 4
        && trimmed
            .chars()
            .all(|character| !character.is_alphanumeric() && !character.is_whitespace())
}

fn is_noise_stub_line(line: &str) -> bool {
    let trimmed = line.trim();
    let chars = trimmed.chars().collect::<Vec<_>>();

    match chars.len() {
        0 => true,
        1 => chars[0].is_alphanumeric(),
        2 => chars.iter().all(|character| character.is_ascii_uppercase()),
        _ => false,
    }
}

fn merge_display_texts(events: &[DisplayTextEvent]) -> Vec<OcrCue> {
    let mut cues = Vec::new();
    let mut active: Option<PendingCue> = None;

    for event in events {
        if event.is_empty {
            if let Some(current) = active.take() {
                cues.push(OcrCue {
                    start_pts: current.start_pts,
                    end_pts: max(current.start_pts + 1, event.pts),
                    text: current.rendered_text,
                });
            }
            continue;
        }

        let Some(text) = event
            .text
            .as_ref()
            .map(|text| text.trim())
            .filter(|text| !text.is_empty())
        else {
            continue;
        };
        let normalized = normalize_for_compare(text);

        match active.take() {
            Some(mut current) if current.normalized_text == normalized => {
                current.last_pts = event.pts;
                active = Some(current);
            }
            Some(mut current)
                if should_extend_pending_cue(&current, text, &normalized, event.pts) =>
            {
                current.last_pts = event.pts;
                current.normalized_text = normalized;
                current.rendered_text = text.to_owned();
                active = Some(current);
            }
            Some(current) => {
                cues.push(OcrCue {
                    start_pts: current.start_pts,
                    end_pts: max(current.start_pts + 1, event.pts),
                    text: current.rendered_text,
                });
                active = Some(PendingCue {
                    start_pts: event.pts,
                    last_pts: event.pts,
                    normalized_text: normalized,
                    rendered_text: text.to_owned(),
                });
            }
            _ => {
                active = Some(PendingCue {
                    start_pts: event.pts,
                    last_pts: event.pts,
                    normalized_text: normalized,
                    rendered_text: text.to_owned(),
                });
            }
        }
    }

    if let Some(current) = active.take() {
        cues.push(OcrCue {
            start_pts: current.start_pts,
            end_pts: current
                .start_pts
                .saturating_add(DEFAULT_LAST_CUE_DURATION_PTS),
            text: current.rendered_text,
        });
    }

    cues
}

fn should_extend_pending_cue(
    current: &PendingCue,
    next_text: &str,
    next_normalized: &str,
    next_pts: u32,
) -> bool {
    if next_pts.saturating_sub(current.last_pts) > MAX_CONTINUATION_GAP_PTS {
        return false;
    }

    if is_fragmentary_text(&current.rendered_text) && !is_fragmentary_text(next_text) {
        return true;
    }

    current.normalized_text.len() >= 2 && next_normalized.contains(&current.normalized_text)
}

fn is_fragmentary_text(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return true;
    }

    let alnum_count = trimmed
        .chars()
        .filter(|character| character.is_alphanumeric())
        .count();
    let visible_count = trimmed
        .chars()
        .filter(|character| !character.is_whitespace())
        .count();
    let token_count = trimmed.split_whitespace().count();

    (alnum_count == 0 && visible_count <= 4) || (token_count <= 1 && alnum_count <= 3)
}

fn render_cues_to_srt(cues: &[OcrCue]) -> String {
    let blocks = cues
        .iter()
        .enumerate()
        .map(|(index, cue)| {
            format!(
                "{}\n{} --> {}\n{}",
                index + 1,
                format_srt_timestamp(cue.start_pts),
                format_srt_timestamp(cue.end_pts),
                cue.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");

    format!("{blocks}\n")
}

fn format_srt_timestamp(pts: u32) -> String {
    let total_milliseconds = ((pts as u64) + (PTS_PER_MILLISECOND / 2)) / PTS_PER_MILLISECOND;
    let hours = total_milliseconds / 3_600_000;
    let minutes = (total_milliseconds % 3_600_000) / 60_000;
    let seconds = (total_milliseconds % 60_000) / 1_000;
    let milliseconds = total_milliseconds % 1_000;

    format!("{hours:02}:{minutes:02}:{seconds:02},{milliseconds:03}")
}

async fn ensure_models(config: &OcrConfig, profile: ResolvedOcrProfile) -> Result<ModelPaths> {
    let bundle = model_bundle(profile);
    let cache_dir = config
        .model_cache_dir
        .clone()
        .unwrap_or_else(default_model_cache_dir)
        .join("paddleocr")
        .join(OCR_MODEL_VERSION)
        .join(profile.cache_segment());
    tokio::fs::create_dir_all(&cache_dir).await?;

    let client = reqwest::Client::builder()
        .timeout(OCR_DOWNLOAD_TIMEOUT)
        .build()?;
    let detection = ensure_model_artifact(&client, &cache_dir, &bundle.detection).await?;
    let recognition = ensure_model_artifact(&client, &cache_dir, &bundle.recognition).await?;
    let charset = ensure_model_artifact(&client, &cache_dir, &bundle.charset).await?;

    Ok(ModelPaths {
        detection,
        recognition,
        charset,
    })
}

async fn ensure_model_artifact(
    client: &reqwest::Client,
    cache_dir: &Path,
    artifact: &ModelArtifact,
) -> Result<PathBuf> {
    let path = cache_dir.join(artifact.file_name);
    if validate_model_artifact(&path, artifact).await? {
        return Ok(path);
    }
    if tokio::fs::metadata(&path).await.is_ok() {
        tokio::fs::remove_file(&path).await?;
    }

    let mut response = client
        .get(artifact.url)
        .send()
        .await?
        .error_for_status()
        .map_err(|error| SubtitleToolkitError::Ocr {
            message: format!(
                "failed to download OCR model {}: {error}",
                artifact.file_name
            ),
        })?;
    if response
        .content_length()
        .is_some_and(|length| length != artifact.size)
    {
        return Err(SubtitleToolkitError::Ocr {
            message: format!(
                "OCR model {} has unexpected Content-Length",
                artifact.file_name
            ),
        });
    }

    let counter = DOWNLOAD_COUNTER.fetch_add(1, AtomicOrdering::Relaxed);
    let temp_path = cache_dir.join(format!(
        ".{}.download-{}-{counter}",
        artifact.file_name,
        std::process::id()
    ));
    let mut file = tokio::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temp_path)
        .await?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    while let Some(chunk) = response.chunk().await? {
        downloaded = downloaded
            .checked_add(chunk.len() as u64)
            .filter(|size| *size <= artifact.size)
            .ok_or_else(|| SubtitleToolkitError::Ocr {
                message: format!("OCR model {} exceeded expected size", artifact.file_name),
            })?;
        hasher.update(&chunk);
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    let digest = format!("{:x}", hasher.finalize());
    if downloaded != artifact.size || digest != artifact.sha256 {
        let _ = tokio::fs::remove_file(&temp_path).await;
        return Err(SubtitleToolkitError::Ocr {
            message: format!(
                "OCR model {} failed size or SHA-256 validation",
                artifact.file_name
            ),
        });
    }

    match tokio::fs::rename(&temp_path, &path).await {
        Ok(()) => Ok(path),
        Err(_) if validate_model_artifact(&path, artifact).await? => {
            tokio::fs::remove_file(&temp_path).await?;
            Ok(path)
        }
        Err(publish_error) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            Err(SubtitleToolkitError::Ocr {
                message: format!(
                    "failed to publish validated OCR model {}: {publish_error}",
                    artifact.file_name
                ),
            })
        }
    }
}

async fn validate_model_artifact(path: &Path, artifact: &ModelArtifact) -> Result<bool> {
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.len() != artifact.size {
        return Ok(false);
    }

    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()) == artifact.sha256)
}

fn default_model_cache_dir() -> PathBuf {
    if let Ok(value) = env::var(OCR_MODEL_DIR_ENV_VAR)
        && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }

    if let Ok(value) = env::var("XDG_CACHE_HOME")
        && !value.trim().is_empty()
    {
        return PathBuf::from(value).join("psyche-subtitle-toolkit");
    }

    if let Ok(home) = env::var("HOME")
        && !home.trim().is_empty()
    {
        return PathBuf::from(home)
            .join(".cache")
            .join("psyche-subtitle-toolkit");
    }

    env::temp_dir().join("psyche-subtitle-toolkit")
}

fn resolve_profile(
    requested: OcrLanguage,
    stream_language: Option<&str>,
    source_language_hint: Option<&str>,
) -> ResolvedOcrProfile {
    match requested {
        OcrLanguage::English => ResolvedOcrProfile::English,
        OcrLanguage::Latin => ResolvedOcrProfile::Latin,
        OcrLanguage::Auto => {
            if stream_language
                .is_some_and(|language| matches_language(language, &["eng", "en"][..]))
                || source_language_hint
                    .is_some_and(|language| normalize_language_name(language).contains("english"))
            {
                ResolvedOcrProfile::English
            } else {
                ResolvedOcrProfile::Latin
            }
        }
    }
}

fn matches_language(value: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| value.trim().eq_ignore_ascii_case(candidate))
}

fn normalize_language_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
}

fn model_bundle(profile: ResolvedOcrProfile) -> ModelBundle {
    let detection = ModelArtifact {
        file_name: "PP-OCRv5_mobile_det.mnn",
        url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/\
            fb21fefb3ace6090ef6fbdda56ae1560dc2a0bae/models/PP-OCRv5_mobile_det.mnn",
        size: 4_760_244,
        sha256: "326f846bb5c903282e116ea089e8796b67921586726cca9457730436a79684c3",
    };

    let (recognition, charset) = match profile {
        ResolvedOcrProfile::English => (
            ModelArtifact {
                file_name: "en_PP-OCRv5_mobile_rec_infer.mnn",
                url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/\
                    fb21fefb3ace6090ef6fbdda56ae1560dc2a0bae/models/en_PP-OCRv5_mobile_rec_infer.mnn",
                size: 3_967_840,
                sha256: "c5e747fb69275e9d99fbe54f97642c65681c3e4244383f825d1d7668e9aece81",
            },
            ModelArtifact {
                file_name: "ppocr_keys_en.txt",
                url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/\
                    fb21fefb3ace6090ef6fbdda56ae1560dc2a0bae/models/ppocr_keys_en.txt",
                size: 1_416,
                sha256: "e025a66d31f327ba0c232e03f407ae8d105e1e709e7ccb3f408aa778c24e70d6",
            },
        ),
        ResolvedOcrProfile::Latin => (
            ModelArtifact {
                file_name: "latin_PP-OCRv5_mobile_rec_infer.mnn",
                url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/\
                    fb21fefb3ace6090ef6fbdda56ae1560dc2a0bae/models/latin_PP-OCRv5_mobile_rec_infer.mnn",
                size: 4_065_440,
                sha256: "f3da21186ade13e0d3553867205a8983d9fda46e10b6a15e5f6308325e6e7a55",
            },
            ModelArtifact {
                file_name: "ppocr_keys_latin.txt",
                url: "https://raw.githubusercontent.com/zibo-chen/rust-paddle-ocr/\
                    fb21fefb3ace6090ef6fbdda56ae1560dc2a0bae/models/ppocr_keys_latin.txt",
                size: 2_616,
                sha256: "ccbcc45730b3fbbd9050c5bc74db6a99067141ef1035e3d14889a84a6b9b1aff",
            },
        ),
    };

    ModelBundle {
        detection,
        recognition,
        charset,
    }
}

impl ResolvedOcrProfile {
    fn cache_segment(self) -> &'static str {
        match self {
            Self::English => "english",
            Self::Latin => "latin",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;
    use imageproc::rect::Rect;
    use pgs_rs::parse::{
        CompositionObject, CompositionState, LastInSequence, ObjectDefinition, PaletteDefinition,
        PaletteEntry, Pgs, PresentationComposition, RlEncodedPixels, RunLengthEncodedData, Segment,
        SegmentContents,
    };

    #[test]
    fn resolve_profile_prefers_english_for_english_hints() {
        assert_eq!(
            resolve_profile(OcrLanguage::Auto, Some("eng"), None),
            ResolvedOcrProfile::English
        );
        assert_eq!(
            resolve_profile(OcrLanguage::Auto, None, Some("English")),
            ResolvedOcrProfile::English
        );
        assert_eq!(
            resolve_profile(OcrLanguage::Auto, Some("por"), Some("Portuguese")),
            ResolvedOcrProfile::Latin
        );
    }

    #[test]
    fn render_rejects_invalid_frame_dimensions() {
        let display_set = ResolvedDisplaySet {
            presentation_timestamp: 0,
            width: 0,
            height: 1080,
            palette_id: 0,
            composition_objects: &[],
            palettes: HashMap::new(),
            objects: HashMap::new(),
        };
        let error = render_display_set_for_ocr(&display_set).unwrap_err();
        assert!(error.to_string().contains("dimensions"));
    }

    #[test]
    fn render_rejects_rle_data_beyond_object_bounds() {
        let composition_objects = [CompositionObject {
            id: 7,
            window_id: 0,
            horizontal_position: 0,
            vertical_position: 0,
            cropped: None,
        }];
        let object = ObjectDefinition {
            id: 7,
            version: 0,
            last_in_sequence: LastInSequence::FirstAndLast,
            width: 2,
            height: 1,
            data: RunLengthEncodedData(vec![RlEncodedPixels { count: 3, color: 1 }]),
        };
        let display_set = ResolvedDisplaySet {
            presentation_timestamp: 0,
            width: 2,
            height: 1,
            palette_id: 0,
            composition_objects: &composition_objects,
            palettes: HashMap::new(),
            objects: HashMap::from([(7, &object)]),
        };
        let error = render_display_set_for_ocr(&display_set).unwrap_err();
        assert!(error.to_string().contains("run-length data exceeds"));
    }

    #[tokio::test]
    async fn cached_model_requires_expected_size_and_sha256() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("model.bin");
        tokio::fs::write(&path, b"abc").await.unwrap();
        let artifact = ModelArtifact {
            file_name: "model.bin",
            url: "https://example.invalid/model.bin",
            size: 3,
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert!(validate_model_artifact(&path, &artifact).await.unwrap());

        tokio::fs::write(&path, b"abd").await.unwrap();
        assert!(!validate_model_artifact(&path, &artifact).await.unwrap());
    }

    #[test]
    fn alpha_bounds_finds_visible_subtitle_region() {
        let mut image = RgbaImage::new(10, 10);
        image.put_pixel(4, 5, Rgba([255, 255, 255, MIN_ALPHA + 1]));
        image.put_pixel(5, 6, Rgba([255, 255, 255, MIN_ALPHA + 1]));

        let bounds = alpha_bounds(&image).expect("bounds should exist");
        assert_eq!(bounds.0, 0);
        assert_eq!(bounds.1, 0);
        assert!(bounds.2 >= 6);
        assert!(bounds.3 >= 7);
    }

    #[test]
    fn prepare_ocr_image_preserves_visible_pixels() {
        let mut image = RgbaImage::new(2, 2);
        image.put_pixel(0, 0, Rgba([255, 255, 255, 255]));
        let prepared = prepare_ocr_image(&image);

        assert!(prepared.width() >= 2);
        assert!(prepared.height() >= 2);
        let pixel = prepared.get_pixel(0, 0);
        assert_eq!(pixel[0], 255);
    }

    #[test]
    fn collect_recognized_text_lines_groups_words_by_line() {
        let lines = collect_recognized_text_lines(vec![
            RecognizedTextBlock {
                bbox: TextBox::new(Rect::at(12, 10).of_size(20, 10), 0.99),
                text: "Hello".to_owned(),
            },
            RecognizedTextBlock {
                bbox: TextBox::new(Rect::at(35, 11).of_size(24, 10), 0.99),
                text: "world".to_owned(),
            },
            RecognizedTextBlock {
                bbox: TextBox::new(Rect::at(10, 40).of_size(14, 10), 0.99),
                text: "Tudo".to_owned(),
            },
            RecognizedTextBlock {
                bbox: TextBox::new(Rect::at(27, 41).of_size(16, 10), 0.99),
                text: "bem?".to_owned(),
            },
        ]);

        assert_eq!(
            lines,
            vec!["Hello world".to_owned(), "Tudo bem?".to_owned()]
        );
    }

    #[test]
    fn merge_display_texts_merges_duplicates_and_closes_on_empty() {
        let cues = merge_display_texts(
            &[
                DisplayTextEvent {
                    pts: 9_000,
                    is_empty: false,
                    text: Some("Hello".to_owned()),
                },
                DisplayTextEvent {
                    pts: 18_000,
                    is_empty: false,
                    text: Some("Hello".to_owned()),
                },
                DisplayTextEvent {
                    pts: 27_000,
                    is_empty: true,
                    text: None,
                },
                DisplayTextEvent {
                    pts: 36_000,
                    is_empty: false,
                    text: Some("World".to_owned()),
                },
            ][..],
        );

        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start_pts, 9_000);
        assert_eq!(cues[0].end_pts, 27_000);
        assert_eq!(cues[0].text, "Hello");
        assert_eq!(cues[1].start_pts, 36_000);
        assert_eq!(cues[1].end_pts, 36_000 + DEFAULT_LAST_CUE_DURATION_PTS);
    }

    #[test]
    fn merge_display_texts_promotes_short_fragment_into_following_full_text() {
        let cues = merge_display_texts(
            &[
                DisplayTextEvent {
                    pts: 9_000,
                    is_empty: false,
                    text: Some("?".to_owned()),
                },
                DisplayTextEvent {
                    pts: 18_000,
                    is_empty: false,
                    text: Some("...e algo que voce deve descobrir por conta propria.".to_owned()),
                },
                DisplayTextEvent {
                    pts: 27_000,
                    is_empty: true,
                    text: None,
                },
            ][..],
        );

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start_pts, 9_000);
        assert_eq!(cues[0].end_pts, 27_000);
        assert_eq!(
            cues[0].text,
            "...e algo que voce deve descobrir por conta propria."
        );
    }

    #[test]
    fn merge_display_texts_promotes_progressive_text_expansion() {
        let cues = merge_display_texts(
            &[
                DisplayTextEvent {
                    pts: 9_000,
                    is_empty: false,
                    text: Some("hello".to_owned()),
                },
                DisplayTextEvent {
                    pts: 18_000,
                    is_empty: false,
                    text: Some("hello world".to_owned()),
                },
                DisplayTextEvent {
                    pts: 27_000,
                    is_empty: true,
                    text: None,
                },
            ][..],
        );

        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].start_pts, 9_000);
        assert_eq!(cues[0].end_pts, 27_000);
        assert_eq!(cues[0].text, "hello world");
    }

    #[test]
    fn repair_fragmentary_ocr_lines_drops_leading_punctuation_only_line() {
        let repaired = repair_fragmentary_ocr_lines(vec![
            "?".to_owned(),
            "Por que? Por que voce nao vai-".to_owned(),
        ]);

        assert_eq!(repaired, vec!["Por que? Por que voce nao vai-"]);
    }

    #[test]
    fn repair_fragmentary_ocr_lines_attaches_trailing_punctuation_only_line() {
        let repaired =
            repair_fragmentary_ocr_lines(vec!["Por que voce morreu'".to_owned(), "?".to_owned()]);

        assert_eq!(repaired, vec!["Por que voce morreu'?".to_owned()]);
    }

    #[test]
    fn repair_fragmentary_ocr_lines_drops_single_character_stub_lines() {
        let repaired = repair_fragmentary_ocr_lines(vec![
            "Voce deveria pelo menos checar seu e-mail uma vez por dia".to_owned(),
            "a".to_owned(),
        ]);

        assert_eq!(
            repaired,
            vec!["Voce deveria pelo menos checar seu e-mail uma vez por dia".to_owned()]
        );
    }

    #[test]
    fn repair_fragmentary_ocr_lines_drops_short_uppercase_stub_lines() {
        let repaired = repair_fragmentary_ocr_lines(vec![
            "Por que ela esta chorando?".to_owned(),
            "IS".to_owned(),
        ]);

        assert_eq!(repaired, vec!["Por que ela esta chorando?".to_owned()]);
    }

    #[test]
    fn repair_fragmentary_ocr_lines_strips_single_leading_punctuation_noise() {
        let repaired = repair_fragmentary_ocr_lines(vec!["? tenho apenas dado".to_owned()]);

        assert_eq!(repaired, vec!["tenho apenas dado".to_owned()]);
    }

    #[test]
    fn format_srt_timestamp_converts_pts_to_clock_time() {
        assert_eq!(format_srt_timestamp(90_000), "00:00:01,000");
        assert_eq!(format_srt_timestamp(135_000), "00:00:01,500");
    }

    #[test]
    fn collect_display_sets_persists_objects_and_palettes_across_display_sets() {
        let pgs = Pgs {
            segments: vec![
                Segment {
                    pts: 9_000,
                    dts: 9_000,
                    contents: SegmentContents::PresentationComposition(PresentationComposition {
                        width: 2,
                        height: 1,
                        frame_rate: 0,
                        composition_number: 1,
                        composition_state: CompositionState::EpochStart,
                        palette_update: false,
                        palette_id: 0,
                        composition_objects: vec![CompositionObject {
                            id: 0,
                            window_id: 0,
                            horizontal_position: 0,
                            vertical_position: 0,
                            cropped: None,
                        }],
                    }),
                },
                Segment {
                    pts: 9_000,
                    dts: 9_000,
                    contents: SegmentContents::PaletteDefinition(PaletteDefinition {
                        id: 0,
                        version: 0,
                        entries: HashMap::from([(
                            1,
                            PaletteEntry {
                                id: 1,
                                luminance: 255,
                                color_difference_red: 128,
                                color_difference_blue: 128,
                                alpha: 255,
                            },
                        )]),
                    }),
                },
                Segment {
                    pts: 9_000,
                    dts: 9_000,
                    contents: SegmentContents::ObjectDefinition(ObjectDefinition {
                        id: 0,
                        version: 0,
                        last_in_sequence: LastInSequence::FirstAndLast,
                        width: 2,
                        height: 1,
                        data: RunLengthEncodedData(vec![RlEncodedPixels { count: 2, color: 1 }]),
                    }),
                },
                Segment {
                    pts: 9_000,
                    dts: 9_000,
                    contents: SegmentContents::End,
                },
                Segment {
                    pts: 18_000,
                    dts: 18_000,
                    contents: SegmentContents::PresentationComposition(PresentationComposition {
                        width: 2,
                        height: 1,
                        frame_rate: 0,
                        composition_number: 2,
                        composition_state: CompositionState::Normal,
                        palette_update: false,
                        palette_id: 0,
                        composition_objects: vec![CompositionObject {
                            id: 0,
                            window_id: 0,
                            horizontal_position: 0,
                            vertical_position: 0,
                            cropped: None,
                        }],
                    }),
                },
                Segment {
                    pts: 18_000,
                    dts: 18_000,
                    contents: SegmentContents::End,
                },
            ],
        };

        let display_sets = collect_display_sets(&pgs).expect("display sets should resolve");
        assert_eq!(display_sets.len(), 2);
        assert!(display_sets[1].objects.contains_key(&0));
        assert!(display_sets[1].palettes.contains_key(&0));

        let image = render_display_set_for_ocr(&display_sets[1])
            .expect("reused object and palette should still render");
        assert_eq!(image.get_pixel(0, 0)[3], 255);
        assert_eq!(image.get_pixel(1, 0)[3], 255);
    }
}
