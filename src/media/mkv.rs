use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;

use crate::error::{Result, SubtitleToolkitError};

#[derive(Debug, Clone, Deserialize)]
pub struct MkvInfo {
    pub tracks: Vec<MkvTrack>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MkvTrack {
    pub id: u64,
    #[serde(rename = "type")]
    pub track_type: String,
    pub codec: Option<String>,
    pub properties: MkvTrackProperties,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MkvTrackProperties {
    pub language: Option<String>,
    pub track_name: Option<String>,
}

impl MkvTrack {
    pub fn is_ass_subtitle(&self) -> bool {
        self.track_type == "subtitles"
            && self.codec.as_deref().is_some_and(|codec| {
                codec.eq_ignore_ascii_case("SubStationAlpha")
                    || codec.eq_ignore_ascii_case("AdvancedSubStationAlpha")
                    || codec.to_ascii_lowercase().contains("substation")
            })
    }
}

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

pub async fn inspect_mkv(path: &Path) -> Result<MkvInfo> {
    let output = run_output("mkvmerge", ["-J".into(), path.as_os_str().into()]).await?;
    Ok(serde_json::from_slice(&output)?)
}

pub fn select_ass_track(info: &MkvInfo, requested_track: Option<u64>) -> Option<&MkvTrack> {
    if let Some(track_id) = requested_track {
        return info
            .tracks
            .iter()
            .find(|track| track.id == track_id && track.is_ass_subtitle());
    }

    info.tracks.iter().find(|track| track.is_ass_subtitle())
}

pub async fn extract_subtitle(input: &Path, track_id: u64, output: &Path) -> Result<()> {
    let selector = format!("{track_id}:{}", output.display());
    run_status(
        "mkvextract",
        [input.as_os_str().into(), "tracks".into(), selector.into()],
    )
    .await
}

pub async fn mux_subtitle_in_place(
    input: &Path,
    replaced_track_id: u64,
    subtitle: &Path,
    language: &str,
) -> Result<()> {
    let parent = input.parent().unwrap_or_else(|| Path::new("."));
    let stem = input
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("output");
    let temp_output = parent.join(format!(".{stem}.psyche-subtitle-toolkit.tmp.mkv"));
    let subtitle_arg = subtitle.as_os_str();
    let language_arg = format!("0:{language}");
    let subtitle_tracks_arg = format!("!{replaced_track_id}");

    run_status(
        "mkvmerge",
        [
            "-o".into(),
            temp_output.as_os_str().into(),
            "--subtitle-tracks".into(),
            subtitle_tracks_arg.into(),
            input.as_os_str().into(),
            "--language".into(),
            language_arg.into(),
            subtitle_arg.into(),
        ],
    )
    .await?;

    tokio::fs::rename(temp_output, input).await?;
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
    let output = Command::new(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await
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
