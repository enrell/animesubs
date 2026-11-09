//! HTTP API tests against a real server bound to an ephemeral loopback port.

mod mock_llm;

use std::path::Path;
use std::time::Duration;

use animesubs_api::{
    ErrorBody, Health, JobList, JobState, JobStatus, ServerInfo, TracksResponse, paths,
};
use animesubs_server::{RunningServer, ServerConfig, ServerError};
use reqwest::StatusCode;
use serde_json::json;

async fn start(home: &Path) -> RunningServer {
    animesubs_server::start(ServerConfig::local(home, 0))
        .await
        .unwrap()
}

fn http() -> reqwest::Client {
    reqwest::Client::builder().no_proxy().build().unwrap()
}

fn authed(server: &RunningServer, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    request.bearer_auth(server.token())
}

async fn error_code(response: reqwest::Response) -> String {
    response.json::<ErrorBody>().await.unwrap().error.code
}

async fn wait_for_terminal(server: &RunningServer, id: &str) -> JobStatus {
    for _ in 0..200 {
        let status: JobStatus =
            authed(server, http().get(format!("{}/v1/jobs/{id}", server.url())))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
        if status.state.is_terminal() {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("job {id} did not finish");
}

#[tokio::test]
async fn health_is_public_and_writes_discovery_file() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;

    let health: Health = http()
        .get(format!("{}/v1/health", server.url()))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(health.service, "animesubs");
    assert_eq!(health.api, 1);

    let info: ServerInfo =
        serde_json::from_slice(&std::fs::read(paths::server_info_file(home.path())).unwrap())
            .unwrap();
    assert_eq!(info.url, server.url());
    assert_eq!(info.pid, std::process::id());
    let token = std::fs::read_to_string(paths::token_file(home.path())).unwrap();
    assert_eq!(token, server.token());

    server.stop().await.unwrap();
    assert!(!paths::server_info_file(home.path()).exists());
}

#[tokio::test]
async fn every_other_endpoint_requires_the_token() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let jobs = format!("{}/v1/jobs", server.url());

    let missing = http().get(&jobs).send().await.unwrap();
    assert_eq!(missing.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(error_code(missing).await, "unauthorized");

    let wrong = http()
        .get(&jobs)
        .bearer_auth("not-the-token")
        .send()
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);

    let shutdown = http()
        .post(format!("{}/v1/shutdown", server.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(shutdown.status(), StatusCode::UNAUTHORIZED);

    let ok = authed(&server, http().get(&jobs)).send().await.unwrap();
    assert_eq!(ok.status(), StatusCode::OK);
    assert!(ok.json::<JobList>().await.unwrap().jobs.is_empty());
    server.stop().await.unwrap();
}

#[tokio::test]
async fn browser_origins_and_foreign_hosts_are_rejected() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let health = format!("{}/v1/health", server.url());

    let from_page = http()
        .get(&health)
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(from_page.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(from_page).await, "forbidden_origin");

    // DNS rebinding: the request reaches 127.0.0.1 but names another host.
    let rebound = authed(&server, http().get(&health))
        .header("Host", "evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(rebound.status(), StatusCode::FORBIDDEN);
    assert_eq!(error_code(rebound).await, "forbidden_host");
    server.stop().await.unwrap();
}

#[tokio::test]
async fn tracks_validates_paths() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let url = format!("{}/v1/tracks", server.url());

    let relative = authed(&server, http().post(&url))
        .json(&json!({ "path": "episode.mkv" }))
        .send()
        .await
        .unwrap();
    assert_eq!(relative.status(), StatusCode::BAD_REQUEST);

    let missing = authed(&server, http().post(&url))
        .json(&json!({ "path": home.path().join("missing.mkv") }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), StatusCode::NOT_FOUND);

    let malformed = authed(&server, http().post(&url))
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    assert_eq!(error_code(malformed).await, "bad_request");
    server.stop().await.unwrap();
}

/// Needs MKVToolNix; skipped when `mkvmerge` is not on PATH.
#[tokio::test]
async fn tracks_lists_subtitles_of_a_real_mkv() {
    if std::process::Command::new("mkvmerge")
        .arg("--version")
        .output()
        .is_err()
    {
        eprintln!("skipping: mkvmerge is not installed");
        return;
    }
    let home = tempfile::tempdir().unwrap();
    let srt = home.path().join("source.srt");
    std::fs::write(&srt, mock_llm::SRT).unwrap();
    let mkv = home.path().join("episode.mkv");
    let status = std::process::Command::new("mkvmerge")
        .arg("-o")
        .arg(&mkv)
        .args(["--language", "0:ja", "--track-name", "0:Dialogue"])
        .arg(&srt)
        .status()
        .unwrap();
    assert!(status.success());

    let server = start(home.path()).await;
    let response: TracksResponse =
        authed(&server, http().post(format!("{}/v1/tracks", server.url())))
            .json(&json!({ "path": mkv }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
    assert_eq!(response.tracks.len(), 1);
    let track = &response.tracks[0];
    assert_eq!(track.format, Some(animesubs_api::SubtitleFormat::Srt));
    assert_eq!(track.name.as_deref(), Some("Dialogue"));
    assert_eq!(response.selected, Some(track.id));
    server.stop().await.unwrap();
}

#[tokio::test]
async fn translate_job_runs_to_completion_with_progress() {
    let llm = mock_llm::start().await;
    let home = tempfile::tempdir().unwrap();
    let input = home.path().join("ep01.srt");
    std::fs::write(&input, mock_llm::SRT).unwrap();
    let server = start(home.path()).await;

    let created = authed(&server, http().post(format!("{}/v1/jobs", server.url())))
        .json(&json!({
            "kind": "translate_subtitle",
            "input": input,
            "target_language": "pt-BR",
            "provider": { "kind": "openai", "model": "test-model", "base_url": llm.uri() }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::ACCEPTED);
    let created: JobStatus = created.json().await.unwrap();
    assert_eq!(created.kind, "translate_subtitle");

    let done = wait_for_terminal(&server, &created.id).await;
    assert_eq!(done.state, JobState::Succeeded, "{:?}", done.error);
    assert_eq!(done.phase, "done");
    let progress = done.progress.unwrap();
    assert_eq!((progress.done, progress.total), (2, 2));

    let output = home.path().join("ep01.pt-BR.srt");
    assert_eq!(done.output.as_deref(), Some(output.as_path()));
    let translated = std::fs::read_to_string(&output).unwrap();
    assert!(translated.contains("PT: Hello there"), "{translated}");
    // Inline formatting survives the round trip through the provider.
    assert!(translated.contains("<i>See you</i>"), "{translated}");
    assert!(translated.contains("00:00:03,000 --> 00:00:04,000"));

    let listed: JobList = authed(&server, http().get(format!("{}/v1/jobs", server.url())))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(listed.jobs.len(), 1);
    server.stop().await.unwrap();
}

#[tokio::test]
async fn provider_failure_fails_the_job_and_keeps_secrets_out_of_status() {
    let llm = mock_llm::start_rejecting().await;
    let home = tempfile::tempdir().unwrap();
    let input = home.path().join("ep01.srt");
    std::fs::write(&input, mock_llm::SRT).unwrap();
    let server = start(home.path()).await;

    let created: JobStatus = authed(&server, http().post(format!("{}/v1/jobs", server.url())))
        .json(&json!({
            "kind": "translate_subtitle",
            "input": input,
            "target_language": "pt-BR",
            "provider": {
                "kind": "openai", "model": "m", "base_url": llm.uri(), "api_key": "sk-very-secret"
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let done = wait_for_terminal(&server, &created.id).await;
    assert_eq!(done.state, JobState::Failed);
    assert_eq!(done.error.as_ref().unwrap().code, "provider_error");
    assert!(!home.path().join("ep01.pt-BR.srt").exists());

    let raw = authed(
        &server,
        http().get(format!("{}/v1/jobs/{}", server.url(), created.id)),
    )
    .send()
    .await
    .unwrap()
    .text()
    .await
    .unwrap();
    assert!(!raw.contains("sk-very-secret"));
    server.stop().await.unwrap();
}

#[tokio::test]
async fn invalid_jobs_are_rejected_before_queueing() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let url = format!("{}/v1/jobs", server.url());
    let video = home.path().join("ep01.mkv");
    std::fs::write(&video, b"x").unwrap();

    let unsupported = authed(&server, http().post(&url))
        .json(&json!({
            "kind": "translate_subtitle",
            "input": video,
            "target_language": "pt-BR",
            "provider": { "kind": "ollama", "model": "m" }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(unsupported.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error_code(unsupported).await, "unsupported");

    let unknown_kind = authed(&server, http().post(&url))
        .json(&json!({ "kind": "mine_bitcoin" }))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_kind.status(), StatusCode::UNPROCESSABLE_ENTITY);

    let unknown_job = authed(&server, http().get(format!("{url}/job-nope")))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown_job.status(), StatusCode::NOT_FOUND);
    server.stop().await.unwrap();
}

#[tokio::test]
async fn shutdown_endpoint_stops_the_server() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let response = authed(
        &server,
        http().post(format!("{}/v1/shutdown", server.url())),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    tokio::time::timeout(Duration::from_secs(5), server.wait())
        .await
        .expect("server did not stop")
        .unwrap();
    assert!(!paths::server_info_file(home.path()).exists());
}

#[tokio::test]
async fn one_server_per_home_and_loopback_only() {
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let second = animesubs_server::start(ServerConfig::local(home.path(), 0)).await;
    match second {
        Err(ServerError::AlreadyRunning { url, .. }) => {
            assert_eq!(url.as_deref(), Some(server.url()))
        }
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("second server started"),
    }
    server.stop().await.unwrap();

    let other_home = tempfile::tempdir().unwrap();
    let mut public = ServerConfig::local(other_home.path(), 0);
    public.addr = "0.0.0.0:0".parse().unwrap();
    assert!(matches!(
        animesubs_server::start(public).await,
        Err(ServerError::NotLoopback(_))
    ));
}

async fn submit(
    server: &RunningServer,
    input: &Path,
    llm: &str,
    api_key: Option<&str>,
) -> JobStatus {
    let mut provider = json!({ "kind": "openai", "model": "m", "base_url": llm });
    if let Some(key) = api_key {
        provider["api_key"] = json!(key);
    }
    let response = authed(server, http().post(format!("{}/v1/jobs", server.url())))
        .json(&json!({
            "kind": "translate_subtitle",
            "input": input,
            "target_language": "pt-BR",
            "provider": provider
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    response.json().await.unwrap()
}

async fn job(server: &RunningServer, id: &str) -> JobStatus {
    authed(server, http().get(format!("{}/v1/jobs/{id}", server.url())))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

async fn cancel(server: &RunningServer, id: &str) -> reqwest::Response {
    authed(
        server,
        http().post(format!("{}/v1/jobs/{id}/cancel", server.url())),
    )
    .send()
    .await
    .unwrap()
}

async fn wait_for_state(server: &RunningServer, id: &str, state: JobState) -> JobStatus {
    for _ in 0..200 {
        let status = job(server, id).await;
        if status.state == state {
            return status;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("job {id} never reached {state:?}");
}

fn write_input(dir: &Path, name: &str) -> std::path::PathBuf {
    let input = dir.join(name);
    std::fs::write(&input, mock_llm::SRT).unwrap();
    input
}

/// Every file in `dir`, sorted by name.
fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn cancel_queued_and_running_jobs_then_the_queue_moves_on() {
    let llm = mock_llm::start_slow(Duration::from_millis(1500)).await;
    let home = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let running = submit(
        &server,
        &write_input(media.path(), "a.srt"),
        &llm.uri(),
        None,
    )
    .await;
    let queued = submit(
        &server,
        &write_input(media.path(), "b.srt"),
        &llm.uri(),
        None,
    )
    .await;
    let next = submit(
        &server,
        &write_input(media.path(), "c.srt"),
        &llm.uri(),
        None,
    )
    .await;
    wait_for_state(&server, &running.id, JobState::Running).await;

    let response = cancel(&server, &queued.id).await;
    assert_eq!(response.status(), StatusCode::OK);
    let cancelled: JobStatus = response.json().await.unwrap();
    assert_eq!(
        (cancelled.state, cancelled.phase.as_str()),
        (JobState::Cancelled, "cancelled")
    );
    assert!(cancelled.finished_at_ms.is_some());
    assert!(cancelled.error.is_none());
    assert!(cancelled.started_at_ms.is_none());

    let response = cancel(&server, &running.id).await;
    assert_eq!(response.status(), StatusCode::OK);
    let stopped: JobStatus = response.json().await.unwrap();
    assert_eq!(stopped.state, JobState::Cancelled);
    assert_eq!(stopped.phase, "cancelled");
    assert!(stopped.error.is_none());

    // Idempotent.
    let again = cancel(&server, &running.id).await;
    assert_eq!(again.status(), StatusCode::OK);
    assert_eq!(again.json::<JobStatus>().await.unwrap(), stopped);

    // The worker moves on to the next job.
    let done = wait_for_terminal(&server, &next.id).await;
    assert_eq!(done.state, JobState::Succeeded, "{:?}", done.error);
    assert_eq!(job(&server, &running.id).await, stopped);
    assert_eq!(
        file_names(media.path()),
        ["a.srt", "b.srt", "c.pt-BR.srt", "c.srt"],
        "cancelled jobs must leave no output or temporary file"
    );

    let finished = cancel(&server, &next.id).await;
    assert_eq!(finished.status(), StatusCode::CONFLICT);
    assert_eq!(error_code(finished).await, "not_cancellable");
    let unknown = cancel(&server, "job-nope").await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    assert_eq!(error_code(unknown).await, "not_found");
    server.stop().await.unwrap();
}

/// Minimal SSE reader: `(event, data)` per frame, `None` at end of stream.
struct Sse {
    response: reqwest::Response,
    buffer: String,
}

impl Sse {
    async fn open(server: &RunningServer, query: &str) -> reqwest::Response {
        authed(
            server,
            http().get(format!("{}/v1/events{query}", server.url())),
        )
        .send()
        .await
        .unwrap()
    }

    fn new(response: reqwest::Response) -> Self {
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["content-type"].to_str().unwrap(),
            "text/event-stream"
        );
        Self {
            response,
            buffer: String::new(),
        }
    }

    async fn next(&mut self) -> Option<(String, String)> {
        loop {
            if let Some(end) = self.buffer.find("\n\n") {
                let frame: String = self.buffer.drain(..end + 2).collect();
                let mut event = String::new();
                let mut data = String::new();
                for line in frame.lines() {
                    if let Some(value) = line.strip_prefix("event: ") {
                        event = value.into();
                    } else if let Some(value) = line.strip_prefix("data: ") {
                        data = value.into();
                    }
                }
                return Some((event, data));
            }
            let chunk = tokio::time::timeout(Duration::from_secs(10), self.response.chunk())
                .await
                .expect("no event within 10 s");
            match chunk {
                Ok(Some(bytes)) => self.buffer.push_str(std::str::from_utf8(&bytes).unwrap()),
                _ => return None,
            }
        }
    }

    async fn next_job(&mut self) -> Option<JobStatus> {
        let (event, data) = self.next().await?;
        assert_eq!(event, "job");
        Some(serde_json::from_str(&data).unwrap())
    }
}

#[tokio::test]
async fn events_stream_a_snapshot_then_every_change_until_the_terminal_frame() {
    let fast = mock_llm::start().await;
    let slow = mock_llm::start_slow(Duration::from_millis(600)).await;
    let home = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;
    let earlier = submit(
        &server,
        &write_input(home.path(), "a.srt"),
        &fast.uri(),
        None,
    )
    .await;
    let earlier = wait_for_terminal(&server, &earlier.id).await;

    let mut all = Sse::new(Sse::open(&server, "").await);
    assert_eq!(all.next_job().await.unwrap(), earlier, "snapshot frame");

    let live = submit(
        &server,
        &write_input(home.path(), "b.srt"),
        &slow.uri(),
        None,
    )
    .await;
    let mut only = Sse::new(Sse::open(&server, &format!("?job={}", live.id)).await);
    let mut seen = Vec::new();
    loop {
        let status = all.next_job().await.unwrap();
        assert_eq!(status.id, live.id, "only the new job changes");
        let terminal = status.state.is_terminal();
        seen.push(status);
        if terminal {
            break;
        }
    }
    let states: Vec<JobState> = seen.iter().map(|status| status.state).collect();
    assert_eq!(states.first(), Some(&JobState::Queued), "{states:?}");
    assert!(states.contains(&JobState::Running), "{states:?}");
    assert_eq!(states.last(), Some(&JobState::Succeeded));
    let phases: Vec<&str> = seen.iter().map(|status| status.phase.as_str()).collect();
    assert!(phases.contains(&"translating"), "{phases:?}");
    assert_eq!(seen.last().unwrap().progress.unwrap().done, 2);
    assert_eq!(seen.last().unwrap(), &job(&server, &live.id).await);

    // The filtered stream sees the same job through to its terminal frame.
    let mut last = None;
    while let Some(status) = only.next_job().await {
        assert_eq!(status.id, live.id);
        let terminal = status.state.is_terminal();
        last = Some(status);
        if terminal {
            break;
        }
    }
    assert_eq!(last.unwrap().state, JobState::Succeeded);

    let unknown = Sse::open(&server, "?job=job-nope").await;
    assert_eq!(unknown.status(), StatusCode::NOT_FOUND);
    let from_page = authed(&server, http().get(format!("{}/v1/events", server.url())))
        .header("Origin", "https://evil.example")
        .send()
        .await
        .unwrap();
    assert_eq!(from_page.status(), StatusCode::FORBIDDEN);
    let anonymous = http()
        .get(format!("{}/v1/events", server.url()))
        .send()
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    // An open stream counts as an attached client.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(server.state().client_idle_ms(), 0);

    // ... and does not keep the server from stopping; the stream then ends.
    let response = authed(
        &server,
        http().post(format!("{}/v1/shutdown", server.url())),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    tokio::time::timeout(Duration::from_secs(5), server.wait())
        .await
        .expect("an open event stream kept the server running")
        .unwrap();
    assert!(all.next().await.is_none());
}

#[tokio::test]
async fn jobs_are_persisted_and_unfinished_ones_come_back_interrupted() {
    let fast = mock_llm::start().await;
    let stuck = mock_llm::start_slow(Duration::from_secs(30)).await;
    let home = tempfile::tempdir().unwrap();
    let media = tempfile::tempdir().unwrap();
    let server = start(home.path()).await;

    let finished = submit(
        &server,
        &write_input(media.path(), "a.srt"),
        &fast.uri(),
        None,
    )
    .await;
    let finished = wait_for_terminal(&server, &finished.id).await;
    let input = write_input(media.path(), "b.srt");
    let running = submit(&server, &input, &stuck.uri(), Some("sk-very-secret")).await;
    wait_for_state(&server, &running.id, JobState::Running).await;

    let file = paths::job_file(home.path(), &running.id);
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(!saved.contains("sk-very-secret"), "{saved}");
    let saved: animesubs_api::PersistedJob = serde_json::from_str(&saved).unwrap();
    assert_eq!(saved.status.state, JobState::Running);
    let animesubs_api::JobSpec::TranslateSubtitle(spec) = &saved.spec;
    assert_eq!(spec.input, input);
    assert_eq!(spec.provider.api_key, None);
    assert_eq!(spec.provider.model.as_deref(), Some("m"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    server.stop().await.unwrap();
    std::fs::write(
        paths::jobs_dir(home.path()).join("job-corrupt.json"),
        "{oops",
    )
    .unwrap();

    let server = start(home.path()).await;
    let listed: JobList = authed(&server, http().get(format!("{}/v1/jobs", server.url())))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let ids: Vec<&str> = listed.jobs.iter().map(|job| job.id.as_str()).collect();
    assert_eq!(ids, [finished.id.as_str(), running.id.as_str()]);
    assert_eq!(listed.jobs[0], finished);
    let interrupted = &listed.jobs[1];
    assert_eq!(interrupted.state, JobState::Failed);
    assert_eq!(interrupted.phase, "failed");
    assert_eq!(interrupted.error.as_ref().unwrap().code, "interrupted");
    assert!(interrupted.finished_at_ms.is_some());
    let saved: animesubs_api::PersistedJob =
        serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(&saved.status, interrupted);
    assert!(!media.path().join("b.pt-BR.srt").exists());

    // Finished jobs can be cancelled no more after a restart than before.
    let response = cancel(&server, &running.id).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    server.stop().await.unwrap();
}
