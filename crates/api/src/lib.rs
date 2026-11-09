//! Wire contract between the animesubs local server and its clients
//! (the desktop app, the CLI, scripts and other automation).
//!
//! Everything a client needs to talk to a server lives here: the JSON types,
//! the API version, the error codes and the files a running server leaves in
//! its home directory so clients can find it. See `docs/ARCHITECTURE.md`.

use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub mod paths;

/// `service` value reported by `GET /v1/health`.
pub const SERVICE_NAME: &str = "animesubs";
/// Version of the HTTP API described by this crate. Bumped on breaking changes.
pub const API_VERSION: u32 = 1;
/// Port used by `animesubs-cli serve` when none is given.
pub const DEFAULT_PORT: u16 = 9650;
/// Bundle identifier of the desktop app; names the per-user home directory.
pub const APP_IDENTIFIER: &str = "com.enrell.animesubs";
/// Overrides the home directory (server state, token, logs).
pub const HOME_ENV: &str = "ANIMESUBS_HOME";
/// Overrides the API token for both the server and clients.
pub const TOKEN_ENV: &str = "ANIMESUBS_TOKEN";

/// Response of `GET /v1/health`, the only unauthenticated endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Health {
    pub service: String,
    pub version: String,
    pub api: u32,
}

/// Body of `POST /v1/tracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracksRequest {
    /// Absolute path to a media file, as seen by the server.
    pub path: PathBuf,
}

/// Response of `POST /v1/tracks`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TracksResponse {
    pub path: PathBuf,
    pub tracks: Vec<SubtitleTrack>,
    /// Track the engine would translate when no track is requested.
    pub selected: Option<u64>,
}

/// One subtitle track of a media file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubtitleTrack {
    /// Container track ID (mkvmerge numbering, the same one mkvextract uses).
    pub id: u64,
    pub codec: Option<String>,
    /// `None` when the engine cannot translate this codec.
    pub format: Option<SubtitleFormat>,
    pub language: Option<String>,
    pub name: Option<String>,
    pub default: bool,
    pub forced: bool,
}

/// Subtitle formats the engine understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SubtitleFormat {
    Ass,
    Srt,
    Vtt,
    /// Bitmap subtitles; translated after OCR.
    Pgs,
}

/// Translation backends provided by the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Ollama,
    /// Any OpenAI-compatible `/v1/chat/completions` endpoint
    /// (OpenAI, LM Studio, llama.cpp, NVIDIA NIM, MiniMax, ...).
    Openai,
    OpencodeZen,
    OpencodeGo,
    Openrouter,
    Anthropic,
    Deepl,
    Google,
    Gemini,
}

impl ProviderKind {
    pub const ALL: [ProviderKind; 9] = [
        ProviderKind::Ollama,
        ProviderKind::Openai,
        ProviderKind::OpencodeZen,
        ProviderKind::OpencodeGo,
        ProviderKind::Openrouter,
        ProviderKind::Anthropic,
        ProviderKind::Deepl,
        ProviderKind::Google,
        ProviderKind::Gemini,
    ];

    /// Stable wire name, also accepted on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            ProviderKind::Ollama => "ollama",
            ProviderKind::Openai => "openai",
            ProviderKind::OpencodeZen => "opencode-zen",
            ProviderKind::OpencodeGo => "opencode-go",
            ProviderKind::Openrouter => "openrouter",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Deepl => "deepl",
            ProviderKind::Google => "google",
            ProviderKind::Gemini => "gemini",
        }
    }

    /// Provider-specific environment variable holding the API key, if any.
    pub fn api_key_env(self) -> Option<&'static str> {
        match self {
            ProviderKind::Ollama => None,
            ProviderKind::Openai => Some("OPENAI_API_KEY"),
            ProviderKind::OpencodeZen => Some("OPENCODE_ZEN_API_KEY"),
            ProviderKind::OpencodeGo => Some("OPENCODE_GO_API_KEY"),
            ProviderKind::Openrouter => Some("OPENROUTER_API_KEY"),
            ProviderKind::Anthropic => Some("ANTHROPIC_API_KEY"),
            ProviderKind::Deepl => Some("DEEPL_API_KEY"),
            ProviderKind::Google => Some("GOOGLE_API_KEY"),
            ProviderKind::Gemini => Some("GEMINI_API_KEY"),
        }
    }
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProviderKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ProviderKind::ALL
            .into_iter()
            .find(|kind| kind.as_str() == value)
            .ok_or_else(|| {
                let names: Vec<&str> = ProviderKind::ALL.iter().map(|k| k.as_str()).collect();
                format!(
                    "unknown provider `{value}`, expected one of: {}",
                    names.join(", ")
                )
            })
    }
}

/// Which backend translates a job, and how to reach it.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Origin of the provider API, without the `/v1/...` suffix
    /// (e.g. `http://localhost:1234` for LM Studio).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    /// Sent by clients; the server never echoes it back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
}

impl fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("kind", &self.kind)
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .field("api_key", &self.api_key.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

/// Body of `POST /v1/jobs`. Tagged by `kind` so new job kinds can be added
/// without breaking existing clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobSpec {
    /// Translate a standalone `.ass`/`.ssa`/`.srt`/`.vtt` file into a new file.
    TranslateSubtitle(TranslateSubtitleJob),
}

impl JobSpec {
    pub fn kind(&self) -> &'static str {
        match self {
            JobSpec::TranslateSubtitle(_) => "translate_subtitle",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TranslateSubtitleJob {
    /// Absolute path of the source subtitle file.
    pub input: PathBuf,
    /// Absolute output path. Defaults to `<input stem>.<target_language>.<ext>`
    /// next to the input.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<PathBuf>,
    pub target_language: String,
    pub provider: ProviderConfig,
    /// Concurrent chunk requests. Defaults to 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parallel: Option<usize>,
    /// Replace `output` if it already exists.
    #[serde(default)]
    pub overwrite: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    /// Stopped by `POST /v1/jobs/{id}/cancel`; nothing was written to `output`.
    Cancelled,
}

impl JobState {
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            JobState::Succeeded | JobState::Failed | JobState::Cancelled
        )
    }
}

/// Translated cues out of translatable cues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

/// Response of `POST /v1/jobs`, `GET /v1/jobs/{id}` and items of `GET /v1/jobs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobStatus {
    pub id: String,
    pub kind: String,
    pub state: JobState,
    /// Current step: `queued`, `starting`, `parsing`, `translating`, `writing`,
    /// then `done`, `failed` or `cancelled`. Clients must accept unknown phases.
    pub phase: String,
    pub progress: Option<Progress>,
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub target_language: String,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub finished_at_ms: Option<u64>,
    pub error: Option<ApiError>,
}

/// `event:` name of the frames of `GET /v1/events`; their `data:` is a [`JobStatus`].
pub const JOB_EVENT: &str = "job";

/// Response of `GET /v1/jobs`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobList {
    pub jobs: Vec<JobStatus>,
}

/// Contents of `<home>/jobs/<id>.json` (see [`paths::job_file`]): what a server
/// keeps of a job across restarts. `spec` never carries the provider's `api_key`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersistedJob {
    pub status: JobStatus,
    pub spec: JobSpec,
}

/// Machine-readable error carried by every non-2xx response and failed job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

/// Envelope of every non-2xx response: `{"error": {"code": ..., "message": ...}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    pub error: ApiError,
}

/// Values of [`ApiError::code`].
pub mod error_codes {
    pub const UNAUTHORIZED: &str = "unauthorized";
    pub const FORBIDDEN_HOST: &str = "forbidden_host";
    pub const FORBIDDEN_ORIGIN: &str = "forbidden_origin";
    pub const BAD_REQUEST: &str = "bad_request";
    pub const NOT_FOUND: &str = "not_found";
    pub const ALREADY_EXISTS: &str = "already_exists";
    /// `POST /v1/jobs/{id}/cancel` on a job that already succeeded or failed (409).
    pub const NOT_CANCELLABLE: &str = "not_cancellable";
    /// The server stopped while the job was queued or running; reported after a restart.
    pub const INTERRUPTED: &str = "interrupted";
    pub const UNSUPPORTED: &str = "unsupported";
    pub const MISSING_TOOL: &str = "missing_tool";
    pub const PROVIDER: &str = "provider_error";
    pub const ENGINE: &str = "engine_error";
    pub const INTERNAL: &str = "internal";
}

/// Contents of `server.json`, written by a running server so clients can find it.
/// The token is kept in a separate file (see [`paths::token_file`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerInfo {
    pub service: String,
    pub version: String,
    pub api: u32,
    /// Base URL, e.g. `http://127.0.0.1:9650`.
    pub url: String,
    pub pid: u32,
    pub started_at_ms: u64,
}

/// `Authorization` header value for a token.
pub fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_round_trips_through_wire_and_cli_names() {
        for kind in ProviderKind::ALL {
            let json = serde_json::to_string(&kind).unwrap();
            assert_eq!(json, format!("\"{}\"", kind.as_str()));
            assert_eq!(kind.as_str().parse::<ProviderKind>().unwrap(), kind);
        }
        assert!("chatgpt".parse::<ProviderKind>().is_err());
    }

    #[test]
    fn job_spec_is_tagged_by_kind() {
        let spec: JobSpec = serde_json::from_value(serde_json::json!({
            "kind": "translate_subtitle",
            "input": "/media/ep01.ass",
            "target_language": "pt-BR",
            "provider": { "kind": "ollama", "model": "qwen3" }
        }))
        .unwrap();
        let JobSpec::TranslateSubtitle(job) = &spec;
        assert_eq!(spec.kind(), "translate_subtitle");
        assert_eq!(job.provider.kind, ProviderKind::Ollama);
        assert!(!job.overwrite);
        assert_eq!(job.output, None);
    }

    #[test]
    fn provider_config_debug_redacts_api_key() {
        let config = ProviderConfig {
            kind: ProviderKind::Openai,
            model: Some("gpt-4o-mini".into()),
            base_url: None,
            api_key: Some("sk-secret".into()),
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("sk-secret"));
        assert!(debug.contains("<redacted>"));
    }

    #[test]
    fn terminal_states() {
        assert!(!JobState::Queued.is_terminal());
        assert!(!JobState::Running.is_terminal());
        assert!(JobState::Succeeded.is_terminal());
        assert!(JobState::Failed.is_terminal());
        assert!(JobState::Cancelled.is_terminal());
        assert_eq!(
            serde_json::to_string(&JobState::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }
}
