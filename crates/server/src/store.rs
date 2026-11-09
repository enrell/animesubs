//! Jobs persisted under `<home>/jobs/<id>.json`.
//!
//! Each file holds a [`PersistedJob`]: the last status and the spec without
//! the provider's API key. Files are written to a temporary name and renamed,
//! with owner-only permissions on Unix. Unreadable files are logged and
//! skipped: a damaged job file never stops the server.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use animesubs_api::{PersistedJob, paths};

const PARTIAL_SUFFIX: &str = ".partial";

/// Directory of persisted jobs.
#[derive(Debug, Clone)]
pub struct JobStore {
    dir: PathBuf,
}

impl JobStore {
    /// Create `<home>/jobs` if needed and remove temporary files a crash left.
    pub fn open(home: &Path) -> io::Result<Self> {
        let dir = paths::jobs_dir(home);
        std::fs::create_dir_all(&dir)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
        }
        for entry in std::fs::read_dir(&dir)?.flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .ends_with(PARTIAL_SUFFIX)
            {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn file(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Every readable job, in no particular order.
    pub fn load(&self) -> Vec<PersistedJob> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) => {
                eprintln!("[jobs] cannot read {}: {error}", self.dir.display());
                return Vec::new();
            }
        };
        let mut jobs = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(stem) = path
                .file_name()
                .and_then(|name| name.to_str())
                .and_then(|name| name.strip_suffix(".json"))
            else {
                continue;
            };
            match read_job(&path) {
                Ok(job) if job.status.id == stem && is_job_id(stem) => jobs.push(job),
                Ok(_) => eprintln!(
                    "[jobs] skipping {}: job id does not match the file name",
                    path.display()
                ),
                Err(error) => eprintln!("[jobs] skipping {}: {error}", path.display()),
            }
        }
        jobs
    }

    /// Write `job` atomically.
    pub fn save(&self, job: &PersistedJob) -> io::Result<()> {
        let id = &job.status.id;
        if !is_job_id(id) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid job id",
            ));
        }
        let data = serde_json::to_vec_pretty(job).map_err(io::Error::other)?;
        let path = self.file(id);
        let partial = self.dir.join(format!("{id}.json{PARTIAL_SUFFIX}"));
        let written =
            write_private(&partial, &data).and_then(|()| std::fs::rename(&partial, &path));
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        written
    }

    pub fn remove(&self, id: &str) {
        if is_job_id(id) {
            let _ = std::fs::remove_file(self.file(id));
        }
    }
}

fn read_job(path: &Path) -> Result<PersistedJob, String> {
    let data = std::fs::read(path).map_err(|error| error.to_string())?;
    serde_json::from_slice(&data).map_err(|error| error.to_string())
}

fn write_private(path: &Path, data: &[u8]) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(data)
}

/// IDs the server generates (`job-<hex>`); anything else is not used as a file name.
fn is_job_id(id: &str) -> bool {
    id.strip_prefix("job-").is_some_and(|rest| {
        !rest.is_empty() && rest.len() <= 64 && rest.bytes().all(|b| b.is_ascii_alphanumeric())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use animesubs_api::{
        JobSpec, JobState, JobStatus, ProviderConfig, ProviderKind, TranslateSubtitleJob,
    };

    fn job(id: &str) -> PersistedJob {
        PersistedJob {
            status: JobStatus {
                id: id.into(),
                kind: "translate_subtitle".into(),
                state: JobState::Succeeded,
                phase: "done".into(),
                progress: None,
                input: "/m/ep01.srt".into(),
                output: Some("/m/ep01.pt-BR.srt".into()),
                target_language: "pt-BR".into(),
                created_at_ms: 1,
                started_at_ms: Some(2),
                finished_at_ms: Some(3),
                error: None,
            },
            spec: JobSpec::TranslateSubtitle(TranslateSubtitleJob {
                input: "/m/ep01.srt".into(),
                output: Some("/m/ep01.pt-BR.srt".into()),
                target_language: "pt-BR".into(),
                provider: ProviderConfig {
                    kind: ProviderKind::Ollama,
                    model: Some("m".into()),
                    base_url: None,
                    api_key: None,
                },
                parallel: Some(1),
                overwrite: false,
            }),
        }
    }

    #[test]
    fn saves_loads_and_skips_bad_files() {
        let home = tempfile::tempdir().unwrap();
        let store = JobStore::open(home.path()).unwrap();
        store.save(&job("job-ab12")).unwrap();
        std::fs::write(store.dir().join("job-bad.json"), "{not json").unwrap();
        std::fs::write(store.dir().join("job-other.json"), {
            serde_json::to_vec(&job("job-ab12")).unwrap()
        })
        .unwrap();
        std::fs::write(store.dir().join("job-x.json.partial"), "half").unwrap();
        assert_eq!(store.load(), vec![job("job-ab12")]);

        let reopened = JobStore::open(home.path()).unwrap();
        assert!(!reopened.dir().join("job-x.json.partial").exists());
        reopened.remove("job-ab12");
        assert!(!reopened.dir().join("job-ab12.json").exists());
        assert!(store.save(&job("../evil")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let home = tempfile::tempdir().unwrap();
        let store = JobStore::open(home.path()).unwrap();
        store.save(&job("job-ab12")).unwrap();
        let mode = |path: &Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&store.dir().join("job-ab12.json")), 0o600);
        assert_eq!(mode(store.dir()), 0o700);
    }
}
