//! Localhost HTTP/JSON server for animesubs.
//!
//! The server owns long-running work (translation jobs) and exposes it to
//! every client the same way: the desktop app, `animesubs-cli`, scripts and
//! agents. It listens on loopback only, requires a bearer token on every
//! endpoint except `GET /v1/health`, and leaves a discovery file
//! (`server.json`) in its home directory so clients can find it.
//!
//! The operations themselves live in [`ops`] so the CLI can run them
//! in-process when no server is wanted.

#![deny(unsafe_code)]

pub mod auth;
mod http;
pub mod jobs;
pub mod ops;
pub mod progress;
pub mod providers;

use std::fs::File;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use animesubs_api::{API_VERSION, SERVICE_NAME, ServerInfo, paths};
use fs2::FileExt;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

pub use http::{AppState, router};

/// Version reported by `GET /v1/health` and `server.json`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// How to start a server.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// Directory holding the token, discovery file, lock and logs.
    pub home: PathBuf,
    /// Listen address. Must be a loopback address; port 0 picks a free port.
    pub addr: SocketAddr,
    /// Use this token instead of the one stored in the home directory.
    pub token: Option<String>,
    /// [`run`] also stops when stdin reaches end-of-file. A parent that spawns
    /// the server with a piped stdin gets it stopped when the parent dies,
    /// even if it is killed before it can call `POST /v1/shutdown`.
    pub stop_on_stdin_eof: bool,
}

impl ServerConfig {
    /// Loopback on `port`, token from the home directory.
    pub fn local(home: impl Into<PathBuf>, port: u16) -> Self {
        Self {
            home: home.into(),
            addr: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            token: None,
            stop_on_stdin_eof: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("refusing to listen on {0}: only loopback addresses are supported")]
    NotLoopback(SocketAddr),
    #[error(
        "another animesubs server is already running for {}{}",
        .home.display(),
        .url.as_deref().map(|url| format!(" at {url}")).unwrap_or_default()
    )]
    AlreadyRunning { home: PathBuf, url: Option<String> },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("could not write server.json: {0}")]
    Discovery(#[from] serde_json::Error),
}

/// A server that is accepting connections.
pub struct RunningServer {
    url: String,
    token: String,
    home: PathBuf,
    shutdown: Arc<Notify>,
    task: JoinHandle<std::io::Result<()>>,
    _lock: File,
}

impl RunningServer {
    /// Base URL, e.g. `http://127.0.0.1:9650`.
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    /// Ask the server to stop accepting requests and exit.
    pub fn shutdown_signal(&self) -> Arc<Notify> {
        self.shutdown.clone()
    }

    /// Wait until the server stops (after `POST /v1/shutdown` or
    /// [`Self::shutdown_signal`]) and remove the discovery file.
    pub async fn wait(self) -> Result<(), ServerError> {
        let result = match self.task.await {
            Ok(result) => result.map_err(ServerError::from),
            Err(error) => Err(ServerError::Io(std::io::Error::other(error))),
        };
        remove_discovery_file(&self.home);
        result
    }

    /// Stop the server and wait for it.
    pub async fn stop(self) -> Result<(), ServerError> {
        self.shutdown.notify_one();
        self.wait().await
    }
}

/// Bind, write `server.json` and start serving in the background.
pub async fn start(config: ServerConfig) -> Result<RunningServer, ServerError> {
    if !config.addr.ip().is_loopback() {
        return Err(ServerError::NotLoopback(config.addr));
    }
    std::fs::create_dir_all(&config.home)?;

    let lock = File::create(paths::lock_file(&config.home))?;
    if lock.try_lock_exclusive().is_err() {
        let url = read_discovery_file(&config.home).map(|info| info.url);
        return Err(ServerError::AlreadyRunning {
            home: config.home,
            url,
        });
    }

    let token = match config.token {
        Some(token) => token,
        None => auth::load_or_create_token(&config.home)?,
    };

    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    let local = listener.local_addr()?;
    let url = format!("http://{local}");

    let shutdown = Arc::new(Notify::new());
    let state = AppState::new(token.clone(), local.port(), shutdown.clone());
    write_discovery_file(
        &config.home,
        &ServerInfo {
            service: SERVICE_NAME.into(),
            version: VERSION.into(),
            api: API_VERSION,
            url: url.clone(),
            pid: std::process::id(),
            started_at_ms: now_ms(),
        },
    )?;

    let signal = shutdown.clone();
    let task = tokio::spawn(async move {
        axum::serve(listener, router(state))
            .with_graceful_shutdown(async move { signal.notified().await })
            .await
    });

    Ok(RunningServer {
        url,
        token,
        home: config.home,
        shutdown,
        task,
        _lock: lock,
    })
}

/// Run a server until `POST /v1/shutdown`, Ctrl-C or SIGTERM.
pub async fn run(config: ServerConfig) -> Result<(), ServerError> {
    let stop_on_stdin_eof = config.stop_on_stdin_eof;
    let server = start(config).await?;
    eprintln!(
        "[serve] listening on {} (home {})",
        server.url(),
        server.home().display()
    );

    let signal = server.shutdown_signal();
    tokio::spawn(async move {
        wait_for_termination().await;
        signal.notify_one();
    });
    if stop_on_stdin_eof {
        let signal = server.shutdown_signal();
        // Blocking read on a plain thread: Tokio's stdin would keep a runtime
        // worker busy and is not cancellable either.
        std::thread::spawn(move || {
            let _ = std::io::copy(&mut std::io::stdin().lock(), &mut std::io::sink());
            eprintln!("[serve] stdin closed, stopping");
            signal.notify_one();
        });
    }

    server.wait().await?;
    eprintln!("[serve] stopped");
    Ok(())
}

async fn wait_for_termination() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Read `server.json` from `home`, if present and well formed.
pub fn read_discovery_file(home: &Path) -> Option<ServerInfo> {
    let data = std::fs::read(paths::server_info_file(home)).ok()?;
    serde_json::from_slice(&data).ok()
}

fn write_discovery_file(home: &Path, info: &ServerInfo) -> Result<(), ServerError> {
    let path = paths::server_info_file(home);
    let partial = path.with_extension("json.partial");
    std::fs::write(&partial, serde_json::to_vec_pretty(info)?)?;
    std::fs::rename(&partial, &path)?;
    Ok(())
}

/// Remove `server.json` if it still describes this process.
fn remove_discovery_file(home: &Path) {
    if read_discovery_file(home).is_some_and(|info| info.pid == std::process::id()) {
        let _ = std::fs::remove_file(paths::server_info_file(home));
    }
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or_default()
}
