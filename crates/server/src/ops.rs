//! Operations shared by the HTTP API and the in-process CLI.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use animesubs_api::{
    ApiError, ProviderConfig, SubtitleFormat, SubtitleTrack, TracksResponse, TranslateSubtitleJob,
    error_codes,
};
use animesubs_engine::media::mkv::{self, MkvTrack};
use animesubs_engine::{
    AssSubtitle, SrtSubtitle, SubtitleDocument, SubtitleToolkitError, Translator, VttSubtitle,
    translate_ass, translate_srt, translate_vtt,
};

use crate::progress::{CountingTranslator, JobProgress};
use crate::providers;

/// Upper bound for concurrent chunk requests of one job.
pub const MAX_PARALLEL: usize = 16;

/// An operation failure with the HTTP status and API error code it maps to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpError {
    pub status: u16,
    pub code: &'static str,
    pub message: String,
}

impl OpError {
    pub fn new(status: u16, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(400, error_codes::BAD_REQUEST, message)
    }

    pub fn to_api(&self) -> ApiError {
        ApiError {
            code: self.code.to_string(),
            message: self.message.clone(),
        }
    }
}

impl std::fmt::Display for OpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

impl std::error::Error for OpError {}

impl From<SubtitleToolkitError> for OpError {
    fn from(error: SubtitleToolkitError) -> Self {
        use SubtitleToolkitError as E;
        let message = error.to_string();
        match error {
            E::MissingTool { .. } => Self::new(503, error_codes::MISSING_TOOL, message),
            E::AssParse { .. } | E::SrtParse { .. } | E::VttParse { .. } => {
                Self::new(422, error_codes::BAD_REQUEST, message)
            }
            E::Provider { .. } | E::Translation { .. } | E::InvalidTranslation { .. } => {
                Self::new(502, error_codes::PROVIDER, message)
            }
            E::Io(ref io) if io.kind() == std::io::ErrorKind::NotFound => {
                Self::new(404, error_codes::NOT_FOUND, message)
            }
            E::CommandFailed { .. } | E::NoSubtitleTrack { .. } | E::NoAssTrack { .. } => {
                Self::new(422, error_codes::ENGINE, message)
            }
            _ => Self::new(500, error_codes::ENGINE, message),
        }
    }
}

fn require_existing_file(path: &Path, what: &str) -> Result<(), OpError> {
    if !path.is_absolute() {
        return Err(OpError::bad_request(format!(
            "{what} must be an absolute path: {}",
            path.display()
        )));
    }
    if !path.is_file() {
        return Err(OpError::new(
            404,
            error_codes::NOT_FOUND,
            format!("{what} not found: {}", path.display()),
        ));
    }
    Ok(())
}

/// List the subtitle tracks of a media file (needs `mkvmerge` on PATH).
pub async fn list_tracks(path: &Path) -> Result<TracksResponse, OpError> {
    require_existing_file(path, "path")?;
    let info = mkv::inspect_mkv(path).await?;
    let selected = mkv::select_subtitle_track(&info, None).map(|(track, _)| track.id);
    let tracks = info
        .tracks
        .iter()
        .filter(|track| track.track_type == "subtitles")
        .map(to_api_track)
        .collect();
    Ok(TracksResponse {
        path: path.to_path_buf(),
        tracks,
        selected,
    })
}

fn to_api_track(track: &MkvTrack) -> SubtitleTrack {
    let format = track.subtitle_format().map(|format| match format {
        mkv::SubtitleFormat::Ass => SubtitleFormat::Ass,
        mkv::SubtitleFormat::Srt => SubtitleFormat::Srt,
        mkv::SubtitleFormat::Vtt => SubtitleFormat::Vtt,
        mkv::SubtitleFormat::Pgs => SubtitleFormat::Pgs,
    });
    let properties = &track.properties;
    SubtitleTrack {
        id: track.id,
        codec: track.codec.clone(),
        format,
        language: properties
            .language_ietf
            .clone()
            .or_else(|| properties.language.clone()),
        name: properties.track_name.clone(),
        default: properties.default_track.unwrap_or(false),
        forced: properties.forced_track.unwrap_or(false),
    }
}

/// Text subtitle formats a translate job reads and writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextFormat {
    Ass,
    Srt,
    Vtt,
}

impl TextFormat {
    fn from_extension(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        match ext.as_str() {
            "ass" | "ssa" => Some(Self::Ass),
            "srt" => Some(Self::Srt),
            "vtt" => Some(Self::Vtt),
            _ => None,
        }
    }
}

/// A validated translate job, ready to run.
#[derive(Debug, Clone)]
pub struct TranslatePlan {
    pub input: PathBuf,
    pub output: PathBuf,
    pub format: TextFormat,
    pub target_language: String,
    pub provider: ProviderConfig,
    pub parallel: usize,
    pub overwrite: bool,
}

/// Validate a translate job without touching the network.
pub fn plan_translate(job: TranslateSubtitleJob) -> Result<TranslatePlan, OpError> {
    require_existing_file(&job.input, "input")?;
    let Some(format) = TextFormat::from_extension(&job.input) else {
        return Err(OpError::new(
            422,
            error_codes::UNSUPPORTED,
            format!(
                "{} is not a .ass/.ssa/.srt/.vtt file; translating subtitles inside video \
                 files through the server is not implemented yet",
                job.input.display()
            ),
        ));
    };

    let target_language = job.target_language.trim().to_string();
    let language_is_safe = !target_language.is_empty()
        && target_language.len() <= 35
        && target_language
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if !language_is_safe {
        return Err(OpError::bad_request(
            "target_language must be a language tag such as `pt-BR`",
        ));
    }

    let output = match job.output {
        Some(output) if !output.is_absolute() => {
            return Err(OpError::bad_request(format!(
                "output must be an absolute path: {}",
                output.display()
            )));
        }
        Some(output) => output,
        None => default_output(&job.input, &target_language),
    };
    if output == job.input {
        return Err(OpError::bad_request("output must differ from input"));
    }
    if output.exists() && !job.overwrite {
        return Err(OpError::new(
            409,
            error_codes::ALREADY_EXISTS,
            format!(
                "{} already exists (set overwrite to replace it)",
                output.display()
            ),
        ));
    }

    let parallel = job.parallel.unwrap_or(1);
    if !(1..=MAX_PARALLEL).contains(&parallel) {
        return Err(OpError::bad_request(format!(
            "parallel must be between 1 and {MAX_PARALLEL}"
        )));
    }

    // Fail fast on a missing model or key instead of after queueing.
    providers::build_translator(&job.provider)?;

    Ok(TranslatePlan {
        input: job.input,
        output,
        format,
        target_language,
        provider: job.provider,
        parallel,
        overwrite: job.overwrite,
    })
}

/// `<dir>/<stem>.<lang>.<ext>`, e.g. `ep01.ass` -> `ep01.pt-BR.ass`.
pub fn default_output(input: &Path, target_language: &str) -> PathBuf {
    let stem = input
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| "subtitle".into());
    let ext = input
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .unwrap_or_else(|| "txt".into());
    input.with_file_name(format!("{stem}.{target_language}.{ext}"))
}

enum Parsed {
    Ass(AssSubtitle),
    Srt(SrtSubtitle),
    Vtt(VttSubtitle),
}

impl Parsed {
    fn document(&self) -> &SubtitleDocument {
        match self {
            Parsed::Ass(sub) => sub.document(),
            Parsed::Srt(sub) => sub.document(),
            Parsed::Vtt(sub) => sub.document(),
        }
    }
}

/// Run a translate plan; returns the written output path.
pub async fn run_translate(
    plan: &TranslatePlan,
    progress: Arc<JobProgress>,
) -> Result<PathBuf, OpError> {
    progress.set_phase("parsing");
    let bytes = tokio::fs::read(&plan.input)
        .await
        .map_err(SubtitleToolkitError::from)?;
    let source = String::from_utf8(bytes).map_err(|_| {
        OpError::new(
            422,
            error_codes::UNSUPPORTED,
            format!("{} is not UTF-8 text", plan.input.display()),
        )
    })?;
    let source = source.strip_prefix('\u{feff}').unwrap_or(&source);
    let parsed = match plan.format {
        TextFormat::Ass => Parsed::Ass(AssSubtitle::parse(source)?),
        TextFormat::Srt => Parsed::Srt(SrtSubtitle::parse(source)?),
        TextFormat::Vtt => Parsed::Vtt(VttSubtitle::parse(source)?),
    };
    let translatable = parsed
        .document()
        .cues
        .iter()
        .filter(|cue| !cue.text.trim().is_empty())
        .count();
    progress.set_total(translatable as u64);

    progress.set_phase("translating");
    let translator: Arc<dyn Translator> = CountingTranslator::wrap(
        providers::build_translator(&plan.provider)?,
        progress.clone(),
    );
    let lang = plan.target_language.as_str();
    let rendered = match parsed {
        Parsed::Ass(sub) => translate_ass(sub, lang, plan.parallel, translator)
            .await?
            .render(),
        Parsed::Srt(sub) => translate_srt(sub, lang, plan.parallel, translator)
            .await?
            .render(),
        Parsed::Vtt(sub) => translate_vtt(sub, lang, plan.parallel, translator)
            .await?
            .render(),
    };

    progress.set_phase("writing");
    write_atomically(&plan.output, rendered.as_bytes(), plan.overwrite).await?;
    progress.complete();
    progress.set_phase("done");
    Ok(plan.output.clone())
}

/// Write next to the destination, then rename, so readers never see a partial file.
async fn write_atomically(path: &Path, data: &[u8], overwrite: bool) -> Result<(), OpError> {
    if !overwrite && tokio::fs::try_exists(path).await.unwrap_or(false) {
        return Err(OpError::new(
            409,
            error_codes::ALREADY_EXISTS,
            format!(
                "{} appeared while translating; not replacing it",
                path.display()
            ),
        ));
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let partial = path.with_file_name(format!(".{name}.{}.partial", std::process::id()));
    let io = |error: std::io::Error| OpError::from(SubtitleToolkitError::from(error));
    tokio::fs::write(&partial, data).await.map_err(io)?;
    if let Err(error) = tokio::fs::rename(&partial, path).await {
        let _ = tokio::fs::remove_file(&partial).await;
        return Err(io(error));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use animesubs_api::ProviderKind;

    fn job(input: PathBuf) -> TranslateSubtitleJob {
        TranslateSubtitleJob {
            input,
            output: None,
            target_language: "pt-BR".into(),
            provider: ProviderConfig {
                kind: ProviderKind::Ollama,
                model: Some("m".into()),
                base_url: Some("http://127.0.0.1:1".into()),
                api_key: None,
            },
            parallel: None,
            overwrite: false,
        }
    }

    #[test]
    fn default_output_sits_next_to_input() {
        assert_eq!(
            default_output(Path::new("/m/ep01.ass"), "pt-BR"),
            PathBuf::from("/m/ep01.pt-BR.ass")
        );
    }

    #[test]
    fn plan_validates_paths_language_and_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let input = dir.path().join("ep01.srt");
        std::fs::write(&input, "1\n00:00:01,000 --> 00:00:02,000\nHi\n").unwrap();

        let plan = plan_translate(job(input.clone())).unwrap();
        assert_eq!(plan.format, TextFormat::Srt);
        assert_eq!(plan.output, dir.path().join("ep01.pt-BR.srt"));
        assert_eq!(plan.parallel, 1);

        let relative = plan_translate(job(PathBuf::from("ep01.srt"))).unwrap_err();
        assert_eq!(relative.code, error_codes::BAD_REQUEST);

        let missing = plan_translate(job(dir.path().join("nope.srt"))).unwrap_err();
        assert_eq!(missing.status, 404);

        let mut bad_lang = job(input.clone());
        bad_lang.target_language = "../pt".into();
        assert_eq!(
            plan_translate(bad_lang).unwrap_err().code,
            error_codes::BAD_REQUEST
        );

        std::fs::write(dir.path().join("ep01.pt-BR.srt"), "taken").unwrap();
        let exists = plan_translate(job(input.clone())).unwrap_err();
        assert_eq!(exists.code, error_codes::ALREADY_EXISTS);

        let mut same = job(input.clone());
        same.output = Some(input.clone());
        same.overwrite = true;
        assert_eq!(
            plan_translate(same).unwrap_err().code,
            error_codes::BAD_REQUEST
        );

        let mut too_parallel = job(input);
        too_parallel.overwrite = true;
        too_parallel.parallel = Some(MAX_PARALLEL + 1);
        assert_eq!(
            plan_translate(too_parallel).unwrap_err().code,
            error_codes::BAD_REQUEST
        );
    }

    #[test]
    fn video_files_are_rejected_for_now() {
        let dir = tempfile::tempdir().unwrap();
        let video = dir.path().join("ep01.mkv");
        std::fs::write(&video, b"not really a video").unwrap();
        let error = plan_translate(job(video)).unwrap_err();
        assert_eq!(error.code, error_codes::UNSUPPORTED);
    }

    #[test]
    fn engine_errors_map_to_api_codes() {
        let missing: OpError = SubtitleToolkitError::MissingTool { tool: "mkvmerge" }.into();
        assert_eq!(
            (missing.status, missing.code),
            (503, error_codes::MISSING_TOOL)
        );
        let parse: OpError = SubtitleToolkitError::SrtParse {
            message: "bad".into(),
        }
        .into();
        assert_eq!(parse.status, 422);
    }

    #[tokio::test]
    async fn list_tracks_requires_an_existing_absolute_path() {
        let error = list_tracks(Path::new("relative.mkv")).await.unwrap_err();
        assert_eq!(error.code, error_codes::BAD_REQUEST);
        let dir = tempfile::tempdir().unwrap();
        let error = list_tracks(&dir.path().join("missing.mkv"))
            .await
            .unwrap_err();
        assert_eq!(error.status, 404);
    }
}
