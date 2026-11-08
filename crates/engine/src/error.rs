use std::io;
use std::path::PathBuf;

/// Errors produced by the subtitle toolkit.
#[derive(Debug, thiserror::Error)]
pub enum SubtitleToolkitError {
    /// Standard I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// HTTP request error (Ollama API).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Required external tool (e.g. `mkvmerge`, `mkvextract`) not found in PATH.
    #[error("required tool `{tool}` was not found in PATH")]
    MissingTool { tool: &'static str },

    /// External command returned a nonzero exit status.
    #[error("command `{program}` failed with status {status}: {stderr}")]
    CommandFailed {
        program: &'static str,
        status: String,
        stderr: String,
    },

    /// External command exceeded the configured execution timeout.
    #[error("command `{program}` timed out after {seconds}s")]
    CommandTimedOut {
        /// Program that timed out.
        program: &'static str,
        /// Timeout in seconds.
        seconds: u64,
    },

    /// No `.mkv` files found at the given path.
    #[error("no MKV files found at {path}")]
    NoMkvFiles { path: PathBuf },

    /// No ASS/SSA subtitle track found in the MKV.
    #[error("no ASS subtitle track found in {path}")]
    NoAssTrack { path: PathBuf },

    /// No supported subtitle track (ASS, SRT, WebVTT, or PGS) found in the MKV.
    #[error("no supported subtitle track found in {path}")]
    NoSubtitleTrack { path: PathBuf },

    /// Malformed ASS subtitle file.
    #[error("ASS parse error: {message}")]
    AssParse { message: String },

    /// Malformed SRT subtitle file.
    #[error("SRT parse error: {message}")]
    SrtParse { message: String },

    /// Malformed WebVTT subtitle file.
    #[error("WebVTT parse error: {message}")]
    VttParse { message: String },

    /// LLM returned invalid numbered text (missing IDs, duplicates, etc.).
    #[error("translation response is invalid: {message}")]
    InvalidTranslation { message: String },

    /// Translation provider returned an error.
    #[error("{provider} error: {message}")]
    Translation {
        provider: &'static str,
        message: String,
    },

    /// Translation provider HTTP failure with retry metadata.
    #[error(
        "{provider} request failed{status_suffix}: {message}",
        status_suffix = status.map(|value| format!(" with HTTP {value}")).unwrap_or_default()
    )]
    Provider {
        /// Provider identifier.
        provider: &'static str,
        /// HTTP status, when the failure came from an HTTP response.
        status: Option<u16>,
        /// Sanitized provider response.
        message: String,
        /// Whether retrying the exact request can reasonably succeed.
        retryable: bool,
        /// Server-requested retry delay in seconds.
        retry_after_seconds: Option<u64>,
    },

    /// Resume manifest is malformed or does not match the requested run.
    #[error("resume progress error: {message}")]
    Progress { message: String },

    /// OCR processing error.
    #[error("OCR error: {message}")]
    Ocr { message: String },

    /// No usable subtitle stream could be selected (ffprobe backend).
    #[error("subtitle selection failed: {message}")]
    SubtitleSelection { message: String },
}

/// Convenience alias for `Result<T, SubtitleToolkitError>`.
pub type Result<T> = std::result::Result<T, SubtitleToolkitError>;
