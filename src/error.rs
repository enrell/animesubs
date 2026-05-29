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

    /// No `.mkv` files found at the given path.
    #[error("no MKV files found at {path}")]
    NoMkvFiles { path: PathBuf },

    /// No ASS/SSA subtitle track found in the MKV.
    #[error("no ASS subtitle track found in {path}")]
    NoAssTrack { path: PathBuf },

    /// Malformed ASS subtitle file.
    #[error("ASS parse error: {message}")]
    AssParse { message: String },

    /// LLM returned invalid numbered text (missing IDs, duplicates, etc.).
    #[error("translation response is invalid: {message}")]
    InvalidTranslation { message: String },

    /// Translation provider returned an error.
    #[error("{provider} error: {message}")]
    Translation {
        provider: &'static str,
        message: String,
    },
}

/// Convenience alias for `Result<T, SubtitleToolkitError>`.
pub type Result<T> = std::result::Result<T, SubtitleToolkitError>;
