//! API token storage and request checks.
//!
//! Threat model: other local users, and web pages in the user's browser that
//! try to reach `127.0.0.1` (directly or through DNS rebinding). The server
//! therefore listens on loopback only, accepts only loopback `Host` headers,
//! rejects any request that carries an `Origin` header (browsers always send
//! one on cross-origin requests; the app and the CLI never do), and requires
//! the bearer token stored with owner-only permissions in the home directory.

use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

use animesubs_api::paths;

const TOKEN_BYTES: usize = 32;

/// Read the token from `<home>/server-token`, creating it on first use.
pub fn load_or_create_token(home: &Path) -> io::Result<String> {
    let path = paths::token_file(home);
    match std::fs::read_to_string(&path) {
        Ok(existing) if is_valid_token(existing.trim()) => return Ok(existing.trim().to_string()),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    let token = generate_token()?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path)?;
    #[cfg(unix)]
    {
        // `mode` only applies when the file is created; tighten a pre-existing one too.
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(token.as_bytes())?;
    file.sync_all()?;
    Ok(token)
}

/// 32 random bytes from the OS, hex encoded.
pub fn generate_token() -> io::Result<String> {
    let mut bytes = [0u8; TOKEN_BYTES];
    getrandom::fill(&mut bytes).map_err(|error| io::Error::other(error.to_string()))?;
    Ok(hex(&bytes))
}

pub(crate) fn random_id(bytes: usize) -> String {
    let mut buffer = vec![0u8; bytes];
    if getrandom::fill(&mut buffer).is_err() {
        // Job IDs only need to be unique within this process.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos())
            .unwrap_or_default();
        return format!("{nanos:x}");
    }
    hex(&buffer)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

fn is_valid_token(token: &str) -> bool {
    token.len() >= 32 && token.bytes().all(|byte| byte.is_ascii_graphic())
}

/// Compare without leaking the position of the first mismatch.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Whether `Authorization` carries `Bearer <expected>`.
pub fn bearer_matches(header: Option<&str>, expected: &str) -> bool {
    header
        .and_then(|value| value.strip_prefix("Bearer "))
        .is_some_and(|token| constant_time_eq(token.trim().as_bytes(), expected.as_bytes()))
}

/// Whether a `Host` header names this server on loopback (defeats DNS rebinding).
pub fn host_allowed(host: Option<&str>, port: u16) -> bool {
    let Some(host) = host else {
        return false;
    };
    ["127.0.0.1", "localhost", "[::1]"]
        .iter()
        .any(|name| host.eq_ignore_ascii_case(&format!("{name}:{port}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_created_once_and_reused() {
        let home = tempfile::tempdir().unwrap();
        let first = load_or_create_token(home.path()).unwrap();
        let second = load_or_create_token(home.path()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.len(), TOKEN_BYTES * 2);
        assert!(first.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    #[cfg(unix)]
    #[test]
    fn token_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        load_or_create_token(home.path()).unwrap();
        let mode = std::fs::metadata(paths::token_file(home.path()))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn empty_or_short_token_file_is_regenerated() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(paths::token_file(home.path()), "short").unwrap();
        let token = load_or_create_token(home.path()).unwrap();
        assert_eq!(token.len(), TOKEN_BYTES * 2);
    }

    #[test]
    fn bearer_check() {
        assert!(bearer_matches(Some("Bearer abc"), "abc"));
        assert!(!bearer_matches(Some("Bearer abd"), "abc"));
        assert!(!bearer_matches(Some("abc"), "abc"));
        assert!(!bearer_matches(Some("Basic abc"), "abc"));
        assert!(!bearer_matches(None, "abc"));
    }

    #[test]
    fn only_loopback_hosts_on_our_port_are_allowed() {
        assert!(host_allowed(Some("127.0.0.1:9650"), 9650));
        assert!(host_allowed(Some("localhost:9650"), 9650));
        assert!(host_allowed(Some("[::1]:9650"), 9650));
        assert!(!host_allowed(Some("127.0.0.1:9651"), 9650));
        assert!(!host_allowed(Some("evil.example:9650"), 9650));
        assert!(!host_allowed(Some("127.0.0.1"), 9650));
        assert!(!host_allowed(None, 9650));
    }
}
