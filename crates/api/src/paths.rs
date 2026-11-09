//! Files a server keeps in its home directory.
//!
//! The default home is the desktop app's config directory
//! (`<config dir>/com.enrell.animesubs`, the same directory Tauri's
//! `app_config_dir()` returns), so the app, the CLI and a server started by
//! either of them agree on where to look without any configuration.

use std::path::{Path, PathBuf};

use crate::{APP_IDENTIFIER, HOME_ENV};

/// `$ANIMESUBS_HOME` if set and non-empty, else `<config dir>/com.enrell.animesubs`.
pub fn default_home() -> Option<PathBuf> {
    match std::env::var_os(HOME_ENV) {
        Some(value) if !value.is_empty() => Some(PathBuf::from(value)),
        _ => dirs::config_dir().map(|dir| dir.join(APP_IDENTIFIER)),
    }
}

/// Discovery file: [`crate::ServerInfo`] of the running server.
pub fn server_info_file(home: &Path) -> PathBuf {
    home.join("server.json")
}

/// API token, created on first start with owner-only permissions.
pub fn token_file(home: &Path) -> PathBuf {
    home.join("server-token")
}

/// Held with an exclusive lock while a server runs: one server per home.
pub fn lock_file(home: &Path) -> PathBuf {
    home.join("server.lock")
}

/// Where a server spawned by a client writes stdout/stderr.
pub fn log_file(home: &Path) -> PathBuf {
    home.join("logs").join("server.log")
}

/// One `<id>.json` per job (status and spec without secrets), so jobs survive restarts.
pub fn jobs_dir(home: &Path) -> PathBuf {
    home.join("jobs")
}

/// Persisted state of one job.
pub fn job_file(home: &Path, id: &str) -> PathBuf {
    jobs_dir(home).join(format!("{id}.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_live_directly_under_home() {
        let home = Path::new("/home/user/.config/com.enrell.animesubs");
        assert_eq!(server_info_file(home), home.join("server.json"));
        assert_eq!(token_file(home), home.join("server-token"));
        assert_eq!(lock_file(home), home.join("server.lock"));
        assert_eq!(log_file(home), home.join("logs/server.log"));
        assert_eq!(job_file(home, "job-1"), home.join("jobs/job-1.json"));
    }
}
