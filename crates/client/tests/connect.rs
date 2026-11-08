//! Discovery and connection logic, against an in-process server or mocks.

use std::time::Duration;

use animesubs_api::{ServerInfo, paths};
use animesubs_client::{ClientError, EnsureOptions, connect, ensure_server};
use animesubs_server::ServerConfig;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn write_server_info(home: &std::path::Path, url: &str) {
    let info = ServerInfo {
        service: "animesubs".into(),
        version: "0.0.0".into(),
        api: 1,
        url: url.into(),
        pid: 1,
        started_at_ms: 0,
    };
    std::fs::write(
        paths::server_info_file(home),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
    std::fs::write(paths::token_file(home), "a".repeat(64)).unwrap();
}

#[tokio::test]
async fn no_discovery_file_means_not_running() {
    let home = tempfile::tempdir().unwrap();
    assert!(matches!(
        connect(home.path()).await,
        Err(ClientError::NotRunning { .. })
    ));
}

#[tokio::test]
async fn stale_discovery_file_means_not_running() {
    let home = tempfile::tempdir().unwrap();
    // Nothing listens on port 9 (discard) on test machines.
    write_server_info(home.path(), "http://127.0.0.1:9");
    assert!(matches!(
        connect(home.path()).await,
        Err(ClientError::NotRunning { .. })
    ));
}

#[tokio::test]
async fn connects_to_a_running_server_and_reports_it_as_not_owned() {
    let home = tempfile::tempdir().unwrap();
    let server = animesubs_server::start(ServerConfig::local(home.path(), 0))
        .await
        .unwrap();

    let client = connect(home.path()).await.unwrap();
    assert_eq!(client.base_url(), server.url());
    assert!(client.jobs().await.unwrap().is_empty());

    // ensure_server must reuse it, not spawn a second one.
    let connection = ensure_server(EnsureOptions {
        server_binary: Some("/nonexistent/animesubs-cli".into()),
        ..EnsureOptions::new(home.path())
    })
    .await
    .unwrap();
    assert!(!connection.is_owned());
    connection.close().await;

    // Closing a borrowed connection leaves the server up.
    assert!(connect(home.path()).await.is_ok());
    server.stop().await.unwrap();
}

#[tokio::test]
async fn wrong_token_is_an_api_error() {
    let home = tempfile::tempdir().unwrap();
    let server = animesubs_server::start(ServerConfig {
        token: Some("b".repeat(64)),
        ..ServerConfig::local(home.path(), 0)
    })
    .await
    .unwrap();
    std::fs::write(paths::token_file(home.path()), "c".repeat(64)).unwrap();

    match connect(home.path()).await {
        Err(ClientError::Api { status, error }) => {
            assert_eq!(status, 401);
            assert_eq!(error.code, "unauthorized");
        }
        other => panic!("expected 401, got {other:?}"),
    }
    server.stop().await.unwrap();
}

#[tokio::test]
async fn other_services_are_incompatible() {
    let impostor = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/health"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "service": "something-else", "version": "9", "api": 1
        })))
        .mount(&impostor)
        .await;
    let home = tempfile::tempdir().unwrap();
    write_server_info(home.path(), &impostor.uri());

    assert!(matches!(
        connect(home.path()).await,
        Err(ClientError::Incompatible { .. })
    ));
}

#[tokio::test]
async fn missing_server_binary_is_reported() {
    let home = tempfile::tempdir().unwrap();
    let result = ensure_server(EnsureOptions {
        server_binary: Some(home.path().join("no-such-binary")),
        ..EnsureOptions::new(home.path())
    })
    .await;
    assert!(
        matches!(result, Err(ClientError::Spawn { .. })),
        "{result:?}"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn server_that_exits_during_startup_is_reported_with_its_log() {
    let home = tempfile::tempdir().unwrap();
    let result = ensure_server(EnsureOptions {
        server_binary: Some("/bin/false".into()),
        startup_timeout: Duration::from_secs(5),
        ..EnsureOptions::new(home.path())
    })
    .await;
    match result {
        Err(ClientError::StartFailed { log, .. }) => assert_eq!(log, paths::log_file(home.path())),
        other => panic!("expected StartFailed, got {other:?}"),
    }
}
