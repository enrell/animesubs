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
