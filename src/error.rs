use std::io;
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum SubtitleToolkitError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("required tool `{tool}` was not found in PATH")]
    MissingTool { tool: &'static str },

    #[error("command `{program}` failed with status {status}: {stderr}")]
    CommandFailed {
        program: &'static str,
        status: String,
        stderr: String,
    },

    #[error("no MKV files found at {path}")]
    NoMkvFiles { path: PathBuf },

    #[error("no ASS subtitle track found in {path}")]
    NoAssTrack { path: PathBuf },

    #[error("ASS parse error: {message}")]
    AssParse { message: String },

    #[error("translation response is invalid: {message}")]
    InvalidTranslation { message: String },

    #[error("Ollama error: {message}")]
    Ollama { message: String },
}

pub type Result<T> = std::result::Result<T, SubtitleToolkitError>;
