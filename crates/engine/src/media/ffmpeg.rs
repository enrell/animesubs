//! Subtitle streams of any container ffmpeg can read, through `ffprobe` and `ffmpeg`.
//!
//! Ported from shinkai-translator (`src/media.rs`, MIT, same author): stream
//! probing, best-stream scoring, extraction and muxing of a translated track.
//! Muxing *adds* a subtitle stream and copies every existing stream, so the
//! original subtitles stay in the output file.
//!
//! The binaries are `ffmpeg`/`ffprobe` from `PATH` unless `ANIMESUBS_FFMPEG` /
//! `ANIMESUBS_FFPROBE` point elsewhere (see [`FfmpegTools`]).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;

use crate::error::{Result, SubtitleToolkitError};
use crate::media::mkv::SubtitleFormat;

/// Overrides the `ffmpeg` binary.
pub const FFMPEG_ENV: &str = "ANIMESUBS_FFMPEG";
/// Overrides the `ffprobe` binary.
pub const FFPROBE_ENV: &str = "ANIMESUBS_FFPROBE";

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Where to find `ffmpeg` and `ffprobe`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FfmpegTools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl Default for FfmpegTools {
    fn default() -> Self {
        Self::from_env()
    }
}

impl FfmpegTools {
    /// `$ANIMESUBS_FFMPEG` / `$ANIMESUBS_FFPROBE`, else `ffmpeg` / `ffprobe` on `PATH`.
    pub fn from_env() -> Self {
        let pick = |name: &str, default: &str| {
            std::env::var_os(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(default))
        };
        Self {
            ffmpeg: pick(FFMPEG_ENV, "ffmpeg"),
            ffprobe: pick(FFPROBE_ENV, "ffprobe"),
        }
    }
}

/// Disposition flags reported by ffprobe.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StreamDisposition {
    pub default: bool,
    pub forced: bool,
    pub hearing_impaired: bool,
}

/// A subtitle stream reported by ffprobe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProbedStream {
    /// Stream index in the container (`-map 0:<index>`).
    pub index: usize,
    /// Position among subtitle streams only (`0:s:<position>`).
    pub subtitle_position: usize,
    pub codec_name: String,
    pub language: Option<String>,
    pub title: Option<String>,
    pub tags: BTreeMap<String, String>,
    pub disposition: StreamDisposition,
}

impl ProbedStream {
    /// How to extract this stream, or `None` if its codec is unsupported.
    pub fn extraction_plan(&self) -> Option<ExtractionPlan> {
        extraction_plan(&self.codec_name)
    }
}

/// How a codec is extracted. [`SubtitleFormat::Pgs`] streams are bitmaps:
/// they are copied to a `.sup` file and need OCR before translation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExtractionPlan {
    pub extension: &'static str,
    /// Value for `-c:s` when extracting.
    pub codec: &'static str,
    pub format: SubtitleFormat,
}

/// Extraction plan for an ffprobe `codec_name`.
pub fn extraction_plan(codec_name: &str) -> Option<ExtractionPlan> {
    let plan = |extension, codec, format| {
        Some(ExtractionPlan {
            extension,
            codec,
            format,
        })
    };
    match codec_name.trim().to_ascii_lowercase().as_str() {
        "ass" | "ssa" => plan("ass", "ass", SubtitleFormat::Ass),
        "subrip" | "srt" => plan("srt", "srt", SubtitleFormat::Srt),
        "webvtt" => plan("vtt", "webvtt", SubtitleFormat::Vtt),
        // MP4/MOV text tracks convert cleanly to SubRip.
        "mov_text" => plan("srt", "srt", SubtitleFormat::Srt),
        "hdmv_pgs_subtitle" => plan("sup", "copy", SubtitleFormat::Pgs),
        _ => None,
    }
}

/// Subtitle streams in `ffprobe -print_format json -show_streams` output.
pub fn parse_ffprobe_output(json: &[u8]) -> Result<Vec<ProbedStream>> {
    let payload: FfprobeOutput = serde_json::from_slice(json)?;
    let mut streams = Vec::new();
    for stream in payload.streams {
        if stream.codec_type.as_deref() != Some("subtitle") {
            continue;
        }
        let tags = stream.tags.unwrap_or_default();
        let disposition = stream.disposition.unwrap_or_default();
        streams.push(ProbedStream {
            index: stream.index,
            subtitle_position: streams.len(),
            codec_name: stream.codec_name.unwrap_or_else(|| "unknown".into()),
            language: tag_value(&tags, &["language"]),
            title: tag_value(&tags, &["title", "handler_name"]),
            tags,
            disposition: StreamDisposition {
                default: disposition.default != 0,
                forced: disposition.forced != 0,
                hearing_impaired: disposition.hearing_impaired != 0,
            },
        });
    }
    Ok(streams)
}

/// Probe the subtitle streams of a media file.
pub async fn probe_subtitle_streams(path: &Path, tools: &FfmpegTools) -> Result<Vec<ProbedStream>> {
    let mut args: Vec<OsString> = ["-v", "error", "-print_format", "json", "-show_streams"]
        .iter()
        .map(OsString::from)
        .collect();
    args.push(path.into());
    let output = run("ffprobe", &tools.ffprobe, args).await?;
    parse_ffprobe_output(&output)
}

/// How good a stream is as the translation source; `None` if unsupported.
///
/// Text formats beat OCR (ASS > SRT > VTT > PGS). The default flag weighs
/// most; forced, hearing-impaired, commentary and signs/songs tracks are
/// penalised.
pub fn score_stream(stream: &ProbedStream) -> Option<i64> {
    let plan = stream.extraction_plan()?;
    let mut score = match plan.format {
        SubtitleFormat::Ass => 30,
        SubtitleFormat::Srt => 20,
        SubtitleFormat::Vtt => 10,
        SubtitleFormat::Pgs => 4,
    };
    if stream.disposition.default {
        score += 400;
    }
    if stream.disposition.forced {
        score -= 350;
    }
    if stream.disposition.hearing_impaired {
        score -= 100;
    }

    let descriptor = searchable_descriptor(stream);
    if contains_any(&descriptor, &["commentary", "comment", "director"]) {
        score -= 500;
    }
    if contains_any(&descriptor, &["sign", "song", "lyrics", "lyric", "karaoke"]) {
        score -= 250;
    }
    Some(score)
}

/// The stream with `requested_index`, or the best-scoring supported one
/// (ties go to the lower index).
pub fn select_subtitle_stream(
    streams: &[ProbedStream],
    requested_index: Option<usize>,
) -> Result<&ProbedStream> {
    let selection_error = |message: String| SubtitleToolkitError::SubtitleSelection { message };

    if let Some(index) = requested_index {
        let stream = streams
            .iter()
            .find(|stream| stream.index == index)
            .ok_or_else(|| selection_error(format!("subtitle stream {index} was not found")))?;
        if stream.extraction_plan().is_none() {
            return Err(selection_error(format!(
                "subtitle stream {index} uses unsupported codec {}",
                stream.codec_name
            )));
        }
        return Ok(stream);
    }

    if streams.is_empty() {
        return Err(selection_error("the file has no subtitle streams".into()));
    }
    streams
        .iter()
        .filter_map(|stream| score_stream(stream).map(|score| (score, stream)))
        .max_by(|left, right| {
            left.0
                .cmp(&right.0)
                .then_with(|| right.1.index.cmp(&left.1.index))
        })
        .map(|(_, stream)| stream)
        .ok_or_else(|| {
            let codecs: Vec<String> = streams.iter().map(describe_stream).collect();
            selection_error(format!(
                "only unsupported subtitle codecs: {}",
                codecs.join(", ")
            ))
        })
}

/// `ffmpeg` arguments that extract `stream` to `output`.
pub fn extract_args(input: &Path, stream: &ProbedStream, output: &Path) -> Result<Vec<OsString>> {
    let plan = stream
        .extraction_plan()
        .ok_or_else(|| SubtitleToolkitError::SubtitleSelection {
            message: format!(
                "subtitle stream {} uses unsupported codec {}",
                stream.index, stream.codec_name
            ),
        })?;
    let map = format!("0:{}", stream.index);
    let mut args: Vec<OsString> = vec!["-v".into(), "error".into(), "-y".into(), "-i".into()];
    args.push(input.into());
    for arg in [
        "-map",
        map.as_str(),
        "-vn",
        "-an",
        "-dn",
        "-c:s",
        plan.codec,
    ] {
        args.push(arg.into());
    }
    args.push(output.into());
    Ok(args)
}

/// Extract one subtitle stream to `output` (use the plan's extension).
pub async fn extract_subtitle_stream(
    input: &Path,
    stream: &ProbedStream,
    output: &Path,
    tools: &FfmpegTools,
) -> Result<()> {
    run(
        "ffmpeg",
        &tools.ffmpeg,
        extract_args(input, stream, output)?,
    )
    .await?;
    Ok(())
}

/// A translated subtitle to add to a video.
#[derive(Clone, Copy, Debug)]
pub struct TranslatedTrack<'a> {
    pub subtitle: &'a Path,
    /// Ass, Srt or Vtt.
    pub format: SubtitleFormat,
    pub target_language: &'a str,
    /// Stream it was translated from: its title and extra tags are carried over.
    pub source: Option<&'a ProbedStream>,
    /// Subtitle streams already in the video; the new one gets this position.
    pub existing_subtitle_streams: usize,
}

/// `ffmpeg` arguments that copy every stream of `video` and add `track`.
pub fn mux_args(
    video: &Path,
    track: &TranslatedTrack<'_>,
    output: &Path,
    overwrite: bool,
) -> Vec<OsString> {
    let position = track.existing_subtitle_streams;
    let language = infer_language_tag(track.target_language);
    let title = translated_title(track.source, track.target_language);

    let mut args: Vec<OsString> = vec![
        "-v".into(),
        "error".into(),
        if overwrite { "-y" } else { "-n" }.into(),
        "-i".into(),
        video.into(),
        "-i".into(),
        track.subtitle.into(),
    ];
    let metadata = format!("-metadata:s:s:{position}");
    let mut push = |values: &[&str]| args.extend(values.iter().map(OsString::from));
    push(&["-map", "0", "-map", "1:0", "-c", "copy"]);
    push(&[
        &format!("-c:s:{position}"),
        mux_subtitle_codec(output, track.format),
    ]);
    push(&[&metadata, &format!("language={language}")]);
    push(&[&metadata, &format!("title={title}")]);
    push(&[&metadata, &format!("handler_name={title}")]);
    for (key, value) in propagated_tags(track.source) {
        push(&[&metadata, &format!("{key}={value}")]);
    }
    args.push(output.into());
    args
}

/// Write `output`: every stream of `video` plus the translated track.
pub async fn mux_subtitle_track(
    video: &Path,
    track: &TranslatedTrack<'_>,
    output: &Path,
    overwrite: bool,
    tools: &FfmpegTools,
) -> Result<()> {
    run(
        "ffmpeg",
        &tools.ffmpeg,
        mux_args(video, track, output, overwrite),
    )
    .await?;
    Ok(())
}

/// Subtitle codec the output container accepts.
pub fn mux_subtitle_codec(output: &Path, format: SubtitleFormat) -> &'static str {
    let extension = output
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "mp4" | "m4v" | "mov" => "mov_text",
        "webm" => "webvtt",
        _ => match format {
            SubtitleFormat::Ass => "ass",
            SubtitleFormat::Vtt => "webvtt",
            SubtitleFormat::Srt | SubtitleFormat::Pgs => "subrip",
        },
    }
}

/// Container language tag for a target language name or code.
pub fn infer_language_tag(target_language: &str) -> String {
    let trimmed = target_language.trim();
    if trimmed.is_empty() {
        return "und".into();
    }
    match normalize_words(trimmed).as_str() {
        "english" => "eng".into(),
        "portuguese" | "portuguese brazil" | "brazilian portuguese" => "por".into(),
        "spanish" | "espanol" => "spa".into(),
        "french" => "fra".into(),
        "german" => "deu".into(),
        "italian" => "ita".into(),
        "japanese" => "jpn".into(),
        "korean" => "kor".into(),
        "chinese" | "mandarin" => "zho".into(),
        _ if is_language_code(trimmed) => trimmed.to_ascii_lowercase(),
        _ => sanitize_language_tag(trimmed),
    }
}

/// `<dir>/<stem>.<lang>.<ext>` next to the input video.
pub fn default_video_output_path(input: &Path, target_language: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("translated");
    let extension = input
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("mkv");
    let target = sanitize_language_tag(target_language);
    input.with_file_name(format!("{stem}.{target}.{extension}"))
}

fn translated_title(source: Option<&ProbedStream>, target_language: &str) -> String {
    let base = source
        .and_then(|stream| {
            stream
                .title
                .clone()
                .filter(|title| !title.trim().is_empty())
                .or_else(|| {
                    stream
                        .language
                        .as_ref()
                        .map(|lang| format!("subtitle {lang}"))
                })
        })
        .unwrap_or_else(|| "subtitle".into());
    format!("{base} [translated {target_language}]")
}

fn propagated_tags(source: Option<&ProbedStream>) -> Vec<(String, String)> {
    let Some(stream) = source else {
        return Vec::new();
    };
    stream
        .tags
        .iter()
        .filter(|(key, value)| {
            !value.trim().is_empty()
                && !matches!(
                    key.trim().to_ascii_lowercase().as_str(),
                    "language" | "title" | "handler_name"
                )
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

fn describe_stream(stream: &ProbedStream) -> String {
    match stream.title.as_deref() {
        Some(title) if !title.trim().is_empty() => format!("{} ({title})", stream.codec_name),
        _ => stream.codec_name.clone(),
    }
}

fn tag_value(tags: &BTreeMap<String, String>, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        tags.iter().find_map(|(key, value)| {
            let value = value.trim();
            (key.eq_ignore_ascii_case(name) && !value.is_empty()).then(|| value.to_string())
        })
    })
}

fn searchable_descriptor(stream: &ProbedStream) -> String {
    let mut descriptor = stream.title.clone().unwrap_or_default();
    if let Some(handler) = tag_value(&stream.tags, &["handler_name"]) {
        descriptor.push(' ');
        descriptor.push_str(&handler);
    }
    normalize_words(&descriptor)
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn is_language_code(value: &str) -> bool {
    let bytes = value.trim().as_bytes();
    matches!(bytes.len(), 2 | 3 | 5) && bytes.iter().all(|b| b.is_ascii_alphabetic() || *b == b'-')
}

fn normalize_words(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_was_space = false;
        } else if !previous_was_space && !normalized.is_empty() {
            normalized.push(' ');
            previous_was_space = true;
        }
    }
    normalized.trim().to_string()
}

fn sanitize_language_tag(target_language: &str) -> String {
    let sanitized: String = target_language
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let sanitized = sanitized.trim_matches('-');
    if sanitized.is_empty() {
        "translated".into()
    } else {
        sanitized.to_string()
    }
}

async fn run(label: &'static str, binary: &Path, args: Vec<OsString>) -> Result<Vec<u8>> {
    let mut command = Command::new(binary);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(COMMAND_TIMEOUT, command.output())
        .await
        .map_err(|_| SubtitleToolkitError::CommandTimedOut {
            program: label,
            seconds: COMMAND_TIMEOUT.as_secs(),
        })?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SubtitleToolkitError::MissingTool { tool: label }
            } else {
                SubtitleToolkitError::Io(error)
            }
        })?;
    if !output.status.success() {
        return Err(SubtitleToolkitError::CommandFailed {
            program: label,
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output.stdout)
}

#[derive(Debug, Deserialize)]
struct FfprobeOutput {
    #[serde(default)]
    streams: Vec<FfprobeStream>,
}

#[derive(Debug, Deserialize)]
struct FfprobeStream {
    index: usize,
    codec_name: Option<String>,
    codec_type: Option<String>,
    #[serde(default)]
    disposition: Option<FfprobeDisposition>,
    #[serde(default)]
    tags: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Default, Deserialize)]
struct FfprobeDisposition {
    #[serde(default)]
    default: i32,
    #[serde(default)]
    forced: i32,
    #[serde(default)]
    hearing_impaired: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream(
        index: usize,
        codec: &str,
        title: Option<&str>,
        disposition: StreamDisposition,
    ) -> ProbedStream {
        let mut tags = BTreeMap::new();
        if let Some(title) = title {
            tags.insert("title".to_string(), title.to_string());
        }
        ProbedStream {
            index,
            subtitle_position: index.saturating_sub(2),
            codec_name: codec.to_string(),
            language: Some("eng".into()),
            title: title.map(ToString::to_string),
            tags,
            disposition,
        }
    }

    fn default_flag() -> StreamDisposition {
        StreamDisposition {
            default: true,
            ..StreamDisposition::default()
        }
    }

    // Ported from shinkai-translator.
    #[test]
    fn prefers_default_supported_text_track() {
        let streams = [
            stream(
                2,
                "hdmv_pgs_subtitle",
                Some("English PGS"),
                StreamDisposition::default(),
            ),
            stream(3, "subrip", Some("English"), default_flag()),
            stream(
                4,
                "ass",
                Some("Signs & Songs"),
                StreamDisposition::default(),
            ),
        ];
        let selected = select_subtitle_stream(&streams, None).unwrap();
        assert_eq!(
            (selected.index, selected.codec_name.as_str()),
            (3, "subrip")
        );
    }

    // Ported from shinkai-translator.
    #[test]
    fn honors_explicit_index() {
        let streams = [stream(
            2,
            "subrip",
            Some("English"),
            StreamDisposition::default(),
        )];
        assert_eq!(select_subtitle_stream(&streams, Some(2)).unwrap().index, 2);
        assert!(matches!(
            select_subtitle_stream(&streams, Some(9)),
            Err(SubtitleToolkitError::SubtitleSelection { .. })
        ));
    }

    // Ported from shinkai-translator.
    #[test]
    fn infers_language_tags() {
        assert_eq!(infer_language_tag("Portuguese (Brazil)"), "por");
        assert_eq!(infer_language_tag("Japanese"), "jpn");
        assert_eq!(infer_language_tag("pt-BR"), "pt-br");
        assert_eq!(infer_language_tag(""), "und");
    }

    #[test]
    fn full_dialogue_beats_signs_and_commentary_and_forced() {
        let streams = [
            stream(
                2,
                "ass",
                Some("Signs & Songs"),
                StreamDisposition::default(),
            ),
            stream(3, "ass", Some("Commentary"), StreamDisposition::default()),
            stream(
                4,
                "ass",
                Some("Forced"),
                StreamDisposition {
                    forced: true,
                    ..StreamDisposition::default()
                },
            ),
            stream(
                5,
                "ass",
                Some("Full Subtitles"),
                StreamDisposition::default(),
            ),
        ];
        assert_eq!(select_subtitle_stream(&streams, None).unwrap().index, 5);
    }

    #[test]
    fn ties_go_to_the_first_stream_and_unsupported_codecs_are_reported() {
        let tie = [
            stream(2, "subrip", None, StreamDisposition::default()),
            stream(3, "subrip", None, StreamDisposition::default()),
        ];
        assert_eq!(select_subtitle_stream(&tie, None).unwrap().index, 2);

        let bitmaps = [stream(
            2,
            "dvd_subtitle",
            Some("DVD"),
            StreamDisposition::default(),
        )];
        let error = select_subtitle_stream(&bitmaps, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("dvd_subtitle (DVD)"), "{error}");
        assert!(select_subtitle_stream(&[], None).is_err());
    }

    #[test]
    fn parses_ffprobe_json_keeping_only_subtitles() {
        let json = br#"{"streams": [
            {"index": 0, "codec_name": "h264", "codec_type": "video"},
            {"index": 1, "codec_name": "aac", "codec_type": "audio", "tags": {"language": "jpn"}},
            {"index": 2, "codec_name": "ass", "codec_type": "subtitle",
             "disposition": {"default": 1, "forced": 0, "hearing_impaired": 0},
             "tags": {"language": "eng", "title": " Full ", "BPS": "123"}},
            {"index": 3, "codec_name": "mov_text", "codec_type": "subtitle",
             "tags": {"handler_name": "SubtitleHandler"}}
        ]}"#;
        let streams = parse_ffprobe_output(json).unwrap();
        assert_eq!(streams.len(), 2);
        assert_eq!(streams[0].index, 2);
        assert_eq!(streams[0].subtitle_position, 0);
        assert_eq!(streams[0].title.as_deref(), Some("Full"));
        assert_eq!(streams[0].language.as_deref(), Some("eng"));
        assert!(streams[0].disposition.default);
        assert_eq!(streams[1].subtitle_position, 1);
        assert_eq!(streams[1].title.as_deref(), Some("SubtitleHandler"));
        assert_eq!(
            streams[1].extraction_plan().unwrap().format,
            SubtitleFormat::Srt
        );
    }

    #[test]
    fn extraction_plans_by_codec() {
        assert_eq!(extraction_plan("ASS").unwrap().codec, "ass");
        assert_eq!(extraction_plan("subrip").unwrap().extension, "srt");
        assert_eq!(
            extraction_plan("webvtt").unwrap().format,
            SubtitleFormat::Vtt
        );
        let pgs = extraction_plan("hdmv_pgs_subtitle").unwrap();
        assert_eq!(
            (pgs.extension, pgs.codec, pgs.format),
            ("sup", "copy", SubtitleFormat::Pgs)
        );
        assert!(extraction_plan("dvd_subtitle").is_none());
    }

    #[test]
    fn mux_adds_a_stream_and_keeps_everything_else() {
        let mut source = stream(3, "ass", Some("Full"), default_flag());
        source.tags.insert("BPS".into(), "100".into());
        source.tags.insert("language".into(), "eng".into());
        let track = TranslatedTrack {
            subtitle: Path::new("/t/ep.pt.ass"),
            format: SubtitleFormat::Ass,
            target_language: "Portuguese",
            source: Some(&source),
            existing_subtitle_streams: 2,
        };
        let args: Vec<String> = mux_args(
            Path::new("/v/ep.mkv"),
            &track,
            Path::new("/v/out.mkv"),
            false,
        )
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
        let joined = args.join(" ");
        assert!(
            joined.starts_with("-v error -n -i /v/ep.mkv -i /t/ep.pt.ass -map 0 -map 1:0 -c copy")
        );
        assert!(joined.contains("-c:s:2 ass"), "{joined}");
        assert!(joined.contains("-metadata:s:s:2 language=por"), "{joined}");
        assert!(
            joined.contains("-metadata:s:s:2 title=Full [translated Portuguese]"),
            "{joined}"
        );
        assert!(joined.contains("-metadata:s:s:2 BPS=100"), "{joined}");
        assert!(
            !joined.contains("language=eng"),
            "source language is not copied: {joined}"
        );
        assert_eq!(args.last().map(String::as_str), Some("/v/out.mkv"));
    }

    #[test]
    fn mux_codec_follows_the_container() {
        assert_eq!(
            mux_subtitle_codec(Path::new("a.mp4"), SubtitleFormat::Ass),
            "mov_text"
        );
        assert_eq!(
            mux_subtitle_codec(Path::new("a.webm"), SubtitleFormat::Srt),
            "webvtt"
        );
        assert_eq!(
            mux_subtitle_codec(Path::new("a.mkv"), SubtitleFormat::Srt),
            "subrip"
        );
        assert_eq!(
            mux_subtitle_codec(Path::new("a.mkv"), SubtitleFormat::Ass),
            "ass"
        );
        assert_eq!(
            default_video_output_path(Path::new("/m/ep01.mp4"), "pt-BR"),
            PathBuf::from("/m/ep01.pt-br.mp4")
        );
    }

    /// Needs ffmpeg and ffprobe; skipped when they are not installed.
    #[tokio::test]
    async fn real_ffmpeg_round_trip_extracts_and_adds_a_track() {
        let tools = FfmpegTools::from_env();
        let available = |bin: &Path| {
            std::process::Command::new(bin)
                .arg("-version")
                .output()
                .is_ok()
        };
        if !available(&tools.ffmpeg) || !available(&tools.ffprobe) {
            eprintln!("skipping: ffmpeg/ffprobe not installed");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let srt = dir.path().join("source.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nHello there\n").unwrap();
        let video = dir.path().join("episode.mkv");
        let status = std::process::Command::new(&tools.ffmpeg)
            .args(["-v", "error", "-y", "-i"])
            .arg(&srt)
            .args([
                "-metadata:s:s:0",
                "language=jpn",
                "-metadata:s:s:0",
                "title=Dialogue",
            ])
            .arg(&video)
            .status()
            .unwrap();
        assert!(status.success());

        let streams = probe_subtitle_streams(&video, &tools).await.unwrap();
        assert_eq!(streams.len(), 1);
        let selected = select_subtitle_stream(&streams, None).unwrap();
        assert_eq!(
            selected.extraction_plan().unwrap().format,
            SubtitleFormat::Srt
        );
        assert_eq!(selected.title.as_deref(), Some("Dialogue"));

        let extracted = dir.path().join("extracted.srt");
        extract_subtitle_stream(&video, selected, &extracted, &tools)
            .await
            .unwrap();
        assert!(
            std::fs::read_to_string(&extracted)
                .unwrap()
                .contains("Hello there")
        );

        let translated = dir.path().join("translated.srt");
        std::fs::write(&translated, "1\n00:00:01,000 --> 00:00:02,000\nOlá\n").unwrap();
        let output = dir.path().join("episode.por.mkv");
        let track = TranslatedTrack {
            subtitle: &translated,
            format: SubtitleFormat::Srt,
            target_language: "Portuguese",
            source: Some(selected),
            existing_subtitle_streams: streams.len(),
        };
        mux_subtitle_track(&video, &track, &output, false, &tools)
            .await
            .unwrap();

        let muxed = probe_subtitle_streams(&output, &tools).await.unwrap();
        assert_eq!(muxed.len(), 2, "original track is kept");
        assert_eq!(muxed[0].language.as_deref(), Some("jpn"));
        assert_eq!(muxed[1].language.as_deref(), Some("por"));
        assert_eq!(
            muxed[1].title.as_deref(),
            Some("Dialogue [translated Portuguese]")
        );
    }
}
