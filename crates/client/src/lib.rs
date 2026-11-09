//! Client for the animesubs local server.
//!
//! [`Client`] calls the HTTP API. [`connect`] finds a running server through
//! its discovery file, and [`ensure_server`] starts one when none is running,
//! the way the desktop app does at startup. A server started by
//! [`ensure_server`] is owned by the caller and stopped by
//! [`Connection::close`] (or killed when the [`ServerProcess`] is dropped);
//! a server that was already running is left alone.

#![deny(unsafe_code)]

mod lifecycle;

use std::path::{Path, PathBuf};
use std::time::Duration;

use animesubs_api::{
    ApiError, ErrorBody, Health, JOB_EVENT, JobList, JobSpec, JobStatus, ServerInfo, TOKEN_ENV,
    TracksRequest, TracksResponse, bearer, paths,
};
use serde::de::DeserializeOwned;

pub use lifecycle::{
    Connection, EnsureOptions, SERVER_BIN_ENV, SERVER_BIN_NAME, ServerProcess, connect,
    ensure_server, locate_server_binary,
};

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("no animesubs server is running for {}", .home.display())]
    NotRunning { home: PathBuf },
    #[error("{url} is not a compatible animesubs server (service `{service}`, API v{api})")]
    Incompatible {
        url: String,
        service: String,
        api: u32,
    },
    #[error("server returned HTTP {status}: {} ({})", .error.message, .error.code)]
    Api { status: u16, error: ApiError },
    #[error("request failed: {0}")]
    Transport(String),
    #[error(
        "could not find the `{}` server binary (set {})",
        SERVER_BIN_NAME,
        SERVER_BIN_ENV
    )]
    BinaryNotFound,
    #[error("could not start {}: {source}", .binary.display())]
    Spawn {
        binary: PathBuf,
        source: std::io::Error,
    },
    #[error("server exited during startup ({status}); see {}", .log.display())]
    StartFailed { status: String, log: PathBuf },
    #[error("server did not become ready within {seconds}s; see {}", .log.display())]
    StartTimeout { seconds: u64, log: PathBuf },
    #[error("job {id} did not finish within {seconds}s")]
    WaitTimeout { id: String, seconds: u64 },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed event from the server: {0}")]
    BadEvent(String),
}

pub type Result<T> = std::result::Result<T, ClientError>;

impl From<reqwest::Error> for ClientError {
    fn from(error: reqwest::Error) -> Self {
        // Without the URL: it is always ours, and errors may end up in UI logs.
        ClientError::Transport(error.without_url().to_string())
    }
}

/// The server sends a keep-alive comment every 15 s; silence this long means it is gone.
const EVENT_READ_TIMEOUT: Duration = Duration::from_secs(45);

/// Typed client for API v1.
#[derive(Debug, Clone)]
pub struct Client {
    base: String,
    token: String,
    http: reqwest::Client,
    /// Without the overall request timeout, for `GET /v1/events`.
    streaming: reqwest::Client,
}

impl Client {
    pub fn new(base_url: impl Into<String>, token: impl Into<String>) -> Result<Self> {
        // Never send the token through an HTTP(S)_PROXY from the environment.
        let builder = || {
            reqwest::Client::builder()
                .no_proxy()
                .connect_timeout(Duration::from_secs(2))
        };
        Ok(Self {
            base: base_url.into().trim_end_matches('/').to_string(),
            token: token.into(),
            http: builder().timeout(Duration::from_secs(60)).build()?,
            streaming: builder().read_timeout(EVENT_READ_TIMEOUT).build()?,
        })
    }

    /// Client for the server described by `<home>/server.json`, without
    /// checking that it is alive (see [`connect`] for that).
    pub fn from_home(home: &Path) -> Result<Self> {
        let not_running = || ClientError::NotRunning {
            home: home.to_path_buf(),
        };
        let info = read_server_info(home).ok_or_else(not_running)?;
        let token = read_token(home).ok_or_else(not_running)?;
        Self::new(info.url, token)
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub async fn health(&self) -> Result<Health> {
        let response = self.http.get(self.url("/v1/health")).send().await?;
        decode(response).await
    }

    pub async fn tracks(&self, path: impl Into<PathBuf>) -> Result<TracksResponse> {
        let body = TracksRequest { path: path.into() };
        let response = self
            .authed(self.http.post(self.url("/v1/tracks")))
            .json(&body);
        decode(response.send().await?).await
    }

    pub async fn submit(&self, spec: &JobSpec) -> Result<JobStatus> {
        let request = self.authed(self.http.post(self.url("/v1/jobs"))).json(spec);
        decode(request.send().await?).await
    }

    pub async fn job(&self, id: &str) -> Result<JobStatus> {
        let request = self.authed(self.http.get(self.url(&format!("/v1/jobs/{id}"))));
        decode(request.send().await?).await
    }

    pub async fn jobs(&self) -> Result<Vec<JobStatus>> {
        let request = self.authed(self.http.get(self.url("/v1/jobs")));
        Ok(decode::<JobList>(request.send().await?).await?.jobs)
    }

    /// Cancel a queued or running job; returns it as `cancelled`. Cancelling a
    /// cancelled job is a no-op; a finished one fails with `not_cancellable` (409).
    pub async fn cancel_job(&self, id: &str) -> Result<JobStatus> {
        let path = format!("/v1/jobs/{id}/cancel");
        let request = self.authed(self.http.post(self.url(&path)));
        decode(request.send().await?).await
    }

    /// Subscribe to `GET /v1/events`: every job's status now, then each update.
    /// With `job`, only that job's.
    pub async fn events(&self, job: Option<&str>) -> Result<JobEvents> {
        let mut request = self.authed(self.streaming.get(self.url("/v1/events")));
        if let Some(job) = job {
            request = request.query(&[("job", job)]);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            return Err(api_error(response).await);
        }
        Ok(JobEvents {
            response,
            buffer: Vec::new(),
        })
    }

    /// Ask the server to exit. Running jobs are abandoned.
    pub async fn shutdown(&self) -> Result<()> {
        let request = self.authed(self.http.post(self.url("/v1/shutdown")));
        let response = request.send().await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(api_error(response).await)
        }
    }

    /// Poll a job until it succeeds or fails, calling `on_update` on each poll.
    pub async fn wait_for_job(
        &self,
        id: &str,
        poll: Duration,
        timeout: Duration,
        mut on_update: impl FnMut(&JobStatus),
    ) -> Result<JobStatus> {
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let status = self.job(id).await?;
            on_update(&status);
            if status.state.is_terminal() {
                return Ok(status);
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(ClientError::WaitTimeout {
                    id: id.to_string(),
                    seconds: timeout.as_secs(),
                });
            }
            tokio::time::sleep(poll).await;
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    fn authed(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        request.header(reqwest::header::AUTHORIZATION, bearer(&self.token))
    }
}

/// Job updates from [`Client::events`].
#[derive(Debug)]
pub struct JobEvents {
    response: reqwest::Response,
    buffer: Vec<u8>,
}

impl JobEvents {
    /// The next job status; `None` once the server ends the stream (it is stopping).
    pub async fn next(&mut self) -> Result<Option<JobStatus>> {
        loop {
            while let Some(frame) = take_frame(&mut self.buffer) {
                if let Some(status) = parse_frame(&frame)? {
                    return Ok(Some(status));
                }
            }
            match self.response.chunk().await? {
                Some(bytes) => self.buffer.extend_from_slice(&bytes),
                None => return Ok(None),
            }
        }
    }
}

/// Remove and return the first complete frame (up to a blank line).
fn take_frame(buffer: &mut Vec<u8>) -> Option<String> {
    let find = |separator: &[u8]| {
        let at = buffer
            .windows(separator.len())
            .position(|w| w == separator)?;
        Some((at, at + separator.len()))
    };
    let (end, next) = match (find(b"\n\n"), find(b"\r\n\r\n")) {
        (Some(lf), Some(crlf)) => lf.min(crlf),
        (lf, crlf) => lf.or(crlf)?,
    };
    let frame = String::from_utf8_lossy(&buffer[..end]).into_owned();
    buffer.drain(..next);
    Some(frame)
}

/// The job status in a frame; `None` for comments and other events.
fn parse_frame(frame: &str) -> Result<Option<JobStatus>> {
    let mut event = "message";
    let mut data = Vec::new();
    for line in frame.lines() {
        let (field, value) = line.split_once(':').unwrap_or((line, ""));
        let value = value.strip_prefix(' ').unwrap_or(value);
        match field {
            "event" => event = value,
            "data" => data.push(value),
            _ => {}
        }
    }
    if event != JOB_EVENT || data.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(&data.join("\n"))
        .map(Some)
        .map_err(|error| ClientError::BadEvent(error.to_string()))
}

async fn decode<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    if response.status().is_success() {
        Ok(response.json().await?)
    } else {
        Err(api_error(response).await)
    }
}

async fn api_error(response: reqwest::Response) -> ClientError {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let error = serde_json::from_str::<ErrorBody>(&text)
        .map(|body| body.error)
        .unwrap_or_else(|_| ApiError {
            code: "http_error".into(),
            message: text.chars().take(500).collect(),
        });
    ClientError::Api { status, error }
}

/// `<home>/server.json`, if present and well formed.
pub fn read_server_info(home: &Path) -> Option<ServerInfo> {
    let data = std::fs::read(paths::server_info_file(home)).ok()?;
    serde_json::from_slice(&data).ok()
}

/// `$ANIMESUBS_TOKEN`, else the token file in `home`.
pub fn read_token(home: &Path) -> Option<String> {
    if let Some(token) = std::env::var(TOKEN_ENV)
        .ok()
        .filter(|t| !t.trim().is_empty())
    {
        return Some(token.trim().to_string());
    }
    let token = std::fs::read_to_string(paths::token_file(home)).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_split_on_blank_lines_and_skip_comments() {
        let status = r#"{"id":"job-1","kind":"translate_subtitle","state":"cancelled",
"phase":"cancelled","progress":null,"input":"/a.srt","output":null,"target_language":"pt-BR",
"created_at_ms":1,"started_at_ms":null,"finished_at_ms":2,"error":null}"#
            .replace('\n', "");
        let mut buffer =
            format!(": keep-alive\n\nevent: job\ndata: {status}\n\nevent: job\r\n").into_bytes();

        let comment = take_frame(&mut buffer).unwrap();
        assert_eq!(parse_frame(&comment).unwrap(), None);
        let job = parse_frame(&take_frame(&mut buffer).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(job.state, animesubs_api::JobState::Cancelled);
        assert_eq!(
            take_frame(&mut buffer),
            None,
            "incomplete frame stays buffered"
        );

        assert!(parse_frame("event: job\ndata: {").is_err());
        assert_eq!(parse_frame("event: other\ndata: {}").unwrap(), None);
    }
}
