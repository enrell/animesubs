use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Deserialize;
use tokio::process::Command;

use crate::error::{Result, SubtitleToolkitError};

/// Parsed MKV container information from `mkvmerge -J`.
#[derive(Debug, Clone, Deserialize)]
pub struct MkvInfo {
    /// List of tracks in the MKV.
    pub tracks: Vec<MkvTrack>,
    /// Embedded attachments reported by `mkvmerge`.
    #[serde(default)]
    pub attachments: Vec<serde_json::Value>,
    /// Chapter entries reported by `mkvmerge`.
    #[serde(default)]
    pub chapters: Vec<serde_json::Value>,
    /// Global tags reported by `mkvmerge`.
    #[serde(default)]
    pub global_tags: Vec<serde_json::Value>,
    /// Track tags reported by `mkvmerge`.
    #[serde(default)]
    pub track_tags: Vec<serde_json::Value>,
}

/// A single track inside an MKV container.
#[derive(Debug, Clone, Deserialize)]
pub struct MkvTrack {
    /// Track ID (0-based).
    pub id: u64,
    /// Track type: `"video"`, `"audio"`, or `"subtitles"`.
    #[serde(rename = "type")]
    pub track_type: String,
    /// Codec name (e.g. `"SubStationAlpha"`, `"AVC/H.264/MPEG-4p10"`).
    pub codec: Option<String>,
    /// Track metadata properties.
    pub properties: MkvTrackProperties,
}

/// Metadata properties for an MKV track.
#[derive(Debug, Clone, Deserialize)]
pub struct MkvTrackProperties {
    /// Matroska legacy language code (e.g. `"eng"`, `"jpn"`).
    pub language: Option<String>,
    /// BCP 47/IETF language code when provided by Matroska.
    pub language_ietf: Option<String>,
    /// User-defined track name.
    pub track_name: Option<String>,
    /// Whether this track is enabled.
    pub enabled_track: Option<bool>,
    /// Whether this is the default track of its type.
    pub default_track: Option<bool>,
    /// Whether the track should be forced on playback.
    pub forced_track: Option<bool>,
    /// Accessibility flag for hearing-impaired content.
    pub hearing_impaired: Option<bool>,
    /// Accessibility flag for visually-impaired content.
    pub visual_impaired: Option<bool>,
    /// Accessibility flag for text descriptions.
    pub text_descriptions: Option<bool>,
    /// Whether this track is in the original language.
    pub original: Option<bool>,
    /// Whether this track contains commentary.
    pub commentary: Option<bool>,
}

/// Subtitle format detected in an MKV track.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleFormat {
    /// ASS/SSA subtitle format.
    Ass,
    /// SRT (SubRip) subtitle format.
    Srt,
    /// WebVTT subtitle format.
    Vtt,
    /// PGS (Presentation Graphic Stream) bitmap subtitle format. Requires OCR.
    Pgs,
}

impl MkvTrack {
    /// Returns `true` if this track is an ASS/SSA subtitle.
    pub fn is_ass_subtitle(&self) -> bool {
        self.track_type == "subtitles"
            && self.codec.as_deref().is_some_and(|codec| {
                codec.eq_ignore_ascii_case("SubStationAlpha")
                    || codec.eq_ignore_ascii_case("AdvancedSubStationAlpha")
            })
    }

    /// Returns `true` if this track is an SRT (SubRip) subtitle.
    pub fn is_srt_subtitle(&self) -> bool {
        self.track_type == "subtitles"
            && self.codec.as_deref().is_some_and(|codec| {
                codec.eq_ignore_ascii_case("SubRip")
                    || codec.eq_ignore_ascii_case("SRT")
                    || codec.eq_ignore_ascii_case("SubRip/SRT")
            })
    }

    /// Returns `true` if this track is a WebVTT subtitle.
    pub fn is_vtt_subtitle(&self) -> bool {
        self.track_type == "subtitles"
            && self.codec.as_deref().is_some_and(|codec| {
                codec.eq_ignore_ascii_case("WebVTT") || codec.eq_ignore_ascii_case("VTT")
            })
    }

    /// Returns `true` if this track is a PGS (bitmap) subtitle.
    pub fn is_pgs_subtitle(&self) -> bool {
        self.track_type == "subtitles"
            && self.codec.as_deref().is_some_and(|codec| {
                codec.eq_ignore_ascii_case("HDMV PGS") || codec.eq_ignore_ascii_case("PGS")
            })
    }

    /// Returns the subtitle format if this track is a supported subtitle.
    pub fn subtitle_format(&self) -> Option<SubtitleFormat> {
        if self.is_ass_subtitle() {
            Some(SubtitleFormat::Ass)
        } else if self.is_srt_subtitle() {
            Some(SubtitleFormat::Srt)
        } else if self.is_vtt_subtitle() {
            Some(SubtitleFormat::Vtt)
        } else if self.is_pgs_subtitle() {
            Some(SubtitleFormat::Pgs)
        } else {
            None
        }
    }
}

/// Discover MKV files at the given path.
///
/// If `input` is a file, returns it as a single-element list.
/// If `input` is a directory, scans for `.mkv` files (non-recursive).
pub async fn discover_mkv_files(input: &Path) -> Result<Vec<PathBuf>> {
    if input.is_file() {
        if is_mkv(input) {
            return Ok(vec![input.to_path_buf()]);
        }
        return Err(SubtitleToolkitError::NoMkvFiles {
            path: input.to_path_buf(),
        });
    }

    let mut files = Vec::new();
    let mut entries = tokio::fs::read_dir(input).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() && is_mkv(&path) {
            files.push(path);
        }
    }
    files.sort();

    if files.is_empty() {
        return Err(SubtitleToolkitError::NoMkvFiles {
            path: input.to_path_buf(),
        });
    }

    Ok(files)
}

/// Inspect an MKV file and return track information.
///
/// Requires `mkvmerge` to be installed and in PATH.
pub async fn inspect_mkv(path: &Path) -> Result<MkvInfo> {
    let output = run_output("mkvmerge", ["-J".into(), path.as_os_str().into()]).await?;
    Ok(serde_json::from_slice(&output)?)
}

/// Select the first ASS subtitle track, or a specific track by ID.
///
/// Returns `None` if no matching ASS track is found.
pub fn select_ass_track(info: &MkvInfo, requested_track: Option<u64>) -> Option<&MkvTrack> {
    if let Some(track_id) = requested_track {
        return info
            .tracks
            .iter()
            .find(|track| track.id == track_id && track.is_ass_subtitle());
    }

    info.tracks.iter().find(|track| track.is_ass_subtitle())
}

/// Select the first supported subtitle track (ASS, SRT, VTT, or PGS), or a specific track by ID.
///
/// Returns the track and its detected format. Prefers ASS > SRT > VTT > PGS when multiple exist.
/// PGS tracks require OCR — the pipeline handles this automatically.
/// Returns `None` if no matching subtitle track is found.
pub fn select_subtitle_track(
    info: &MkvInfo,
    requested_track: Option<u64>,
) -> Option<(&MkvTrack, SubtitleFormat)> {
    if let Some(track_id) = requested_track {
        let track = info.tracks.iter().find(|t| t.id == track_id)?;
        let format = track.subtitle_format()?;
        return Some((track, format));
    }

    // Prefer ASS > SRT > VTT > PGS
    if let Some(track) = info.tracks.iter().find(|t| t.is_ass_subtitle()) {
        return Some((track, SubtitleFormat::Ass));
    }

    if let Some(track) = info.tracks.iter().find(|t| t.is_srt_subtitle()) {
        return Some((track, SubtitleFormat::Srt));
    }

    if let Some(track) = info.tracks.iter().find(|t| t.is_vtt_subtitle()) {
        return Some((track, SubtitleFormat::Vtt));
    }

    info.tracks
        .iter()
        .find(|t| t.is_pgs_subtitle())
        .map(|track| (track, SubtitleFormat::Pgs))
}

/// Extract a subtitle track from an MKV file.
///
/// Requires `mkvextract` to be installed and in PATH.
pub async fn extract_subtitle(input: &Path, track_id: u64, output: &Path) -> Result<()> {
    let selector = format!("{track_id}:{}", output.display());
    run_status(
        "mkvextract",
        [input.as_os_str().into(), "tracks".into(), selector.into()],
    )
    .await
}

/// Mux a subtitle file into an MKV, replacing the specified track.
///
/// Writes to a temporary file first, then renames over the original for safety.
/// Requires `mkvmerge` to be installed and in PATH.
pub async fn mux_subtitle_in_place(
    input: &Path,
    replaced_track_id: u64,
    subtitle: &Path,
    language: &str,
) -> Result<()> {
    let original_info = inspect_mkv(input).await?;
    let replaced_index = original_info
        .tracks
        .iter()
        .position(|track| track.id == replaced_track_id)
        .ok_or_else(|| SubtitleToolkitError::NoSubtitleTrack {
            path: input.to_path_buf(),
        })?;
    let replaced_track = &original_info.tracks[replaced_index];
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let transaction_dir = tempfile::Builder::new()
        .prefix(".psyche-subtitle-toolkit-")
        .tempdir_in(parent)?;
    let temp_output = transaction_dir.path().join("candidate.mkv");
    let backup = transaction_dir.path().join("original.mkv");
    let language_arg = format!("0:{language}");
    let subtitle_tracks_arg = format!("!{replaced_track_id}");
    let track_order = original_info
        .tracks
        .iter()
        .map(|track| {
            if track.id == replaced_track_id {
                "1:0".to_string()
            } else {
                format!("0:{}", track.id)
            }
        })
        .collect::<Vec<_>>()
        .join(",");

    let mut args: Vec<std::ffi::OsString> = vec![
        "-o".into(),
        temp_output.as_os_str().into(),
        "--subtitle-tracks".into(),
        subtitle_tracks_arg.into(),
        input.as_os_str().into(),
        "--language".into(),
        language_arg.into(),
    ];
    push_track_string_option(
        &mut args,
        "--track-name",
        replaced_track.properties.track_name.as_deref(),
    );
    push_track_bool_option(
        &mut args,
        "--track-enabled-flag",
        replaced_track.properties.enabled_track,
    );
    push_track_bool_option(
        &mut args,
        "--default-track-flag",
        replaced_track.properties.default_track,
    );
    push_track_bool_option(
        &mut args,
        "--forced-display-flag",
        replaced_track.properties.forced_track,
    );
    push_track_bool_option(
        &mut args,
        "--hearing-impaired-flag",
        replaced_track.properties.hearing_impaired,
    );
    push_track_bool_option(
        &mut args,
        "--visual-impaired-flag",
        replaced_track.properties.visual_impaired,
    );
    push_track_bool_option(
        &mut args,
        "--text-descriptions-flag",
        replaced_track.properties.text_descriptions,
    );
    push_track_bool_option(
        &mut args,
        "--original-flag",
        replaced_track.properties.original,
    );
    push_track_bool_option(
        &mut args,
        "--commentary-flag",
        replaced_track.properties.commentary,
    );
    args.push(subtitle.as_os_str().into());
    args.push("--track-order".into());
    args.push(track_order.into());

    run_status("mkvmerge", args).await?;
    let candidate_info = inspect_mkv(&temp_output).await?;
    validate_mux_candidate(&original_info, &candidate_info, replaced_index, language)?;

    tokio::fs::rename(input, &backup).await?;
    if let Err(error) = tokio::fs::rename(&temp_output, input).await {
        let rollback = tokio::fs::rename(&backup, input).await;
        return match rollback {
            Ok(()) => Err(error.into()),
            Err(rollback_error) => {
                let preserved_dir = transaction_dir.keep();
                Err(SubtitleToolkitError::Io(std::io::Error::other(format!(
                    "failed to install muxed MKV ({error}) and rollback original ({rollback_error}); \
                        backup remains at {}",
                    preserved_dir.join("original.mkv").display()
                ))))
            }
        };
    }
    if let Err(error) = tokio::fs::remove_file(&backup).await {
        eprintln!(
            "[mux] warning: translated MKV is valid, but backup cleanup failed at {}: {error}",
            backup.display()
        );
    }
    Ok(())
}

fn push_track_string_option(
    args: &mut Vec<std::ffi::OsString>,
    option: &'static str,
    value: Option<&str>,
) {
    if let Some(value) = value {
        args.push(option.into());
        args.push(format!("0:{value}").into());
    }
}

fn push_track_bool_option(
    args: &mut Vec<std::ffi::OsString>,
    option: &'static str,
    value: Option<bool>,
) {
    if let Some(value) = value {
        args.push(option.into());
        args.push(format!("0:{}", u8::from(value)).into());
    }
}

fn validate_mux_candidate(
    original: &MkvInfo,
    candidate: &MkvInfo,
    replaced_index: usize,
    language: &str,
) -> Result<()> {
    if original.tracks.len() != candidate.tracks.len() {
        return Err(SubtitleToolkitError::CommandFailed {
            program: "mkvmerge",
            status: "candidate validation failed".into(),
            stderr: format!(
                "track count changed from {} to {}",
                original.tracks.len(),
                candidate.tracks.len()
            ),
        });
    }
    if original.attachments.len() != candidate.attachments.len()
        || original.chapters.len() != candidate.chapters.len()
        || original.global_tags.len() != candidate.global_tags.len()
        || original.track_tags.len() != candidate.track_tags.len()
    {
        return Err(SubtitleToolkitError::CommandFailed {
            program: "mkvmerge",
            status: "candidate validation failed".into(),
            stderr: "attachments, chapters, or tag counts changed during remux".into(),
        });
    }

    for (index, (before, after)) in original.tracks.iter().zip(&candidate.tracks).enumerate() {
        if index != replaced_index
            && (before.track_type != after.track_type || before.codec != after.codec)
        {
            return Err(SubtitleToolkitError::CommandFailed {
                program: "mkvmerge",
                status: "candidate validation failed".into(),
                stderr: format!("track {index} changed type or codec"),
            });
        }
    }

    let before = &original.tracks[replaced_index];
    let after = &candidate.tracks[replaced_index];
    let language_matches = after
        .properties
        .language_ietf
        .as_deref()
        .or(after.properties.language.as_deref())
        .is_some_and(|value| value.eq_ignore_ascii_case(language));
    let metadata_matches = before.properties.track_name == after.properties.track_name
        && before.properties.enabled_track == after.properties.enabled_track
        && before.properties.default_track == after.properties.default_track
        && before.properties.forced_track == after.properties.forced_track
        && before.properties.hearing_impaired == after.properties.hearing_impaired
        && before.properties.visual_impaired == after.properties.visual_impaired
        && before.properties.text_descriptions == after.properties.text_descriptions
        && before.properties.original == after.properties.original
        && before.properties.commentary == after.properties.commentary;
    if after.track_type != "subtitles" || !language_matches || !metadata_matches {
        return Err(SubtitleToolkitError::CommandFailed {
            program: "mkvmerge",
            status: "candidate validation failed".into(),
            stderr: "replacement subtitle metadata or language was not preserved".into(),
        });
    }
    Ok(())
}

fn is_mkv(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mkv"))
}

async fn run_output(
    program: &'static str,
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<Vec<u8>> {
    const COMMAND_TIMEOUT: Duration = Duration::from_secs(30 * 60);

    let mut command = Command::new(program);
    command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(COMMAND_TIMEOUT, command.output())
        .await
        .map_err(|_| SubtitleToolkitError::CommandTimedOut {
            program,
            seconds: COMMAND_TIMEOUT.as_secs(),
        })?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                SubtitleToolkitError::MissingTool { tool: program }
            } else {
                SubtitleToolkitError::Io(error)
            }
        })?;

    if !output.status.success() {
        return Err(SubtitleToolkitError::CommandFailed {
            program,
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    Ok(output.stdout)
}

async fn run_status(
    program: &'static str,
    args: impl IntoIterator<Item = std::ffi::OsString>,
) -> Result<()> {
    run_output(program, args).await.map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASS_HEADER: &str = "[Script Info]\nScriptType: v4.00+\n\
        \n[V4+ Styles]\nFormat: Name, Fontname, Fontsize\nStyle: Default,Arial,20\n\
        \n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";

    #[tokio::test]
    async fn real_mux_preserves_track_order_name_and_flags() {
        if Command::new("mkvmerge")
            .arg("--version")
            .output()
            .await
            .is_err()
        {
            eprintln!("skipping MKV integration test: mkvmerge is unavailable");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("first.ass");
        let second = dir.path().join("second.ass");
        let translated = dir.path().join("translated.ass");
        let mkv = dir.path().join("sample.mkv");
        tokio::fs::write(
            &first,
            format!("{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\n"),
        )
        .await
        .unwrap();
        tokio::fs::write(
            &second,
            format!("{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Sekai\n"),
        )
        .await
        .unwrap();
        tokio::fs::write(
            &translated,
            format!("{ASS_HEADER}Dialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,OlÃ¡\n"),
        )
        .await
        .unwrap();

        run_status(
            "mkvmerge",
            vec![
                "-o".into(),
                mkv.as_os_str().into(),
                "--language".into(),
                "0:eng".into(),
                "--track-name".into(),
                "0:English Source".into(),
                "--default-track-flag".into(),
                "0:1".into(),
                "--forced-display-flag".into(),
                "0:1".into(),
                first.as_os_str().into(),
                "--language".into(),
                "0:jpn".into(),
                "--track-name".into(),
                "0:Japanese Source".into(),
                "--default-track-flag".into(),
                "0:0".into(),
                second.as_os_str().into(),
            ],
        )
        .await
        .unwrap();

        let before = inspect_mkv(&mkv).await.unwrap();
        assert_eq!(before.tracks.len(), 2);
        let replaced_id = before.tracks[0].id;
        mux_subtitle_in_place(&mkv, replaced_id, &translated, "pt-BR")
            .await
            .unwrap();

        let after = inspect_mkv(&mkv).await.unwrap();
        assert_eq!(after.tracks.len(), 2);
        assert_eq!(
            after.tracks[0].properties.track_name.as_deref(),
            Some("English Source")
        );
        assert_eq!(after.tracks[0].properties.default_track, Some(true));
        assert_eq!(after.tracks[0].properties.forced_track, Some(true));
        assert_eq!(
            after.tracks[0].properties.language_ietf.as_deref(),
            Some("pt-BR")
        );
        assert_eq!(
            after.tracks[1].properties.track_name.as_deref(),
            Some("Japanese Source")
        );
        assert_eq!(
            after.tracks[1].properties.language_ietf.as_deref(),
            Some("ja")
        );
    }
}
