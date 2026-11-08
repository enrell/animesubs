//! Find, start and stop a local server.

use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use animesubs_api::{API_VERSION, SERVICE_NAME, paths};

use crate::{Client, ClientError, Result};

/// Name of the headless binary that runs `serve` (without `.exe`).
pub const SERVER_BIN_NAME: &str = "animesubs-cli";
/// Overrides where [`locate_server_binary`] looks.
pub const SERVER_BIN_ENV: &str = "ANIMESUBS_SERVER_BIN";

/// Connect to the server recorded in `<home>/server.json`.
///
/// Checks that it answers `/v1/health` as a compatible animesubs server and
/// that our token is accepted. A stale discovery file (crashed server) is
/// reported as [`ClientError::NotRunning`].
pub async fn connect(home: &Path) -> Result<Client> {
    let client = Client::from_home(home)?;
    // Any failure here means the discovery file is stale (or the port now
    // belongs to something else): treat it as "no server".
    let health = client.health().await.map_err(|_| ClientError::NotRunning {
        home: home.to_path_buf(),
    })?;
    if health.service != SERVICE_NAME || health.api != API_VERSION {
        return Err(ClientError::Incompatible {
            url: client.base_url().to_string(),
            service: health.service,
            api: health.api,
        });
    }
    client.jobs().await?;
    Ok(client)
}

/// How [`ensure_server`] finds and starts a server.
#[derive(Debug, Clone)]
pub struct EnsureOptions {
    pub home: PathBuf,
    /// Binary to run as `<binary> serve`. Defaults to [`locate_server_binary`].
    pub server_binary: Option<PathBuf>,
    pub startup_timeout: Duration,
}

impl EnsureOptions {
    pub fn new(home: impl Into<PathBuf>) -> Self {
        Self {
            home: home.into(),
            server_binary: None,
            startup_timeout: Duration::from_secs(15),
        }
    }
}

/// A live connection, plus the server process if this call started it.
#[derive(Debug)]
pub struct Connection {
    pub client: Client,
    pub owned: Option<ServerProcess>,
}

impl Connection {
    /// Whether this connection started the server (and will stop it).
    pub fn is_owned(&self) -> bool {
        self.owned.is_some()
    }

    /// Stop the server if we started it; leave a pre-existing one running.
    pub async fn close(self) {
        if let Some(process) = self.owned {
            process.stop(&self.client, Duration::from_secs(5)).await;
        }
    }
}

/// Connect to a running server, or start
/// `<binary> serve --port 0 --home <home> --exit-on-stdin-eof` and wait until
/// it is ready.
pub async fn ensure_server(options: EnsureOptions) -> Result<Connection> {
    match connect(&options.home).await {
        Ok(client) => {
            return Ok(Connection {
                client,
                owned: None,
            });
        }
        Err(ClientError::NotRunning { .. }) => {}
        Err(other) => return Err(other),
    }

    let binary = match options.server_binary {
        Some(binary) => binary,
        None => locate_server_binary().ok_or(ClientError::BinaryNotFound)?,
    };
    std::fs::create_dir_all(&options.home)?;
    let log = paths::log_file(&options.home);
    let mut process = ServerProcess::spawn(&binary, &options.home, &log)?;

    let deadline = tokio::time::Instant::now() + options.startup_timeout;
    loop {
        if let Some(status) = process.try_wait()? {
            // Another server may have won the race for this home; use it.
            if let Ok(client) = connect(&options.home).await {
                return Ok(Connection {
                    client,
                    owned: None,
                });
            }
            return Err(ClientError::StartFailed { status, log });
        }
        match connect(&options.home).await {
            Ok(client) => {
                return Ok(Connection {
                    client,
                    owned: Some(process),
                });
            }
            Err(ClientError::NotRunning { .. }) => {}
            Err(other) => return Err(other),
        }
        if tokio::time::Instant::now() >= deadline {
            process.kill();
            return Err(ClientError::StartTimeout {
                seconds: options.startup_timeout.as_secs(),
                log,
            });
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// `$ANIMESUBS_SERVER_BIN`, else `animesubs-cli` next to the running
/// executable (where Tauri puts bundled sidecars, and where Cargo puts
/// workspace binaries during development), else the first one on `PATH`.
pub fn locate_server_binary() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os(SERVER_BIN_ENV).map(PathBuf::from) {
        return path.is_file().then_some(path);
    }
    let file_name = format!("{SERVER_BIN_NAME}{}", std::env::consts::EXE_SUFFIX);
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        let sibling = dir.join(&file_name);
        if sibling.is_file() {
            return Some(sibling);
        }
    }
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(&file_name))
        .find(|candidate| candidate.is_file())
}

/// A server process started by this client. Killed on drop unless stopped.
#[derive(Debug)]
pub struct ServerProcess {
    child: Option<Child>,
}

impl ServerProcess {
    fn spawn(binary: &Path, home: &Path, log: &Path) -> Result<Self> {
        if let Some(dir) = log.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let log_file = OpenOptions::new().create(true).append(true).open(log)?;
        let mut command = Command::new(binary);
        command
            .arg("serve")
            .arg("--port")
            .arg("0")
            .arg("--home")
            .arg(home)
            // We hold the write end of stdin for the server's lifetime. If this
            // process dies without calling `stop`, the OS closes it and the
            // server shuts itself down instead of lingering.
            .arg("--exit-on-stdin-eof")
            .stdin(Stdio::piped())
            .stdout(log_file.try_clone()?)
            .stderr(log_file);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = command.spawn().map_err(|source| ClientError::Spawn {
            binary: binary.to_path_buf(),
            source,
        })?;
        Ok(Self { child: Some(child) })
    }

    pub fn id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    fn try_wait(&mut self) -> Result<Option<String>> {
        match self.child.as_mut() {
            Some(child) => Ok(child.try_wait()?.map(|status| status.to_string())),
            None => Ok(Some("already stopped".into())),
        }
    }

    /// Ask for a graceful shutdown, then kill after `grace`.
    pub async fn stop(mut self, client: &Client, grace: Duration) {
        let _ = client.shutdown().await;
        let deadline = tokio::time::Instant::now() + grace;
        while tokio::time::Instant::now() < deadline {
            if matches!(self.try_wait(), Ok(Some(_))) {
                self.child = None;
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        self.kill();
    }

    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.kill();
    }
}
