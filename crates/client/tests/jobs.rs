//! Job calls against an in-process server and a mock LLM.

#[path = "../../server/tests/mock_llm/mod.rs"]
mod mock_llm;

use std::time::Duration;

use animesubs_api::{JobSpec, JobState, ProviderConfig, ProviderKind, TranslateSubtitleJob};
use animesubs_client::{Client, ClientError};
use animesubs_server::ServerConfig;

fn spec(input: std::path::PathBuf, llm: &str) -> JobSpec {
    JobSpec::TranslateSubtitle(TranslateSubtitleJob {
        input,
        output: None,
        target_language: "pt-BR".into(),
        provider: ProviderConfig {
            kind: ProviderKind::Openai,
            model: Some("m".into()),
            base_url: Some(llm.into()),
            api_key: None,
        },
        parallel: None,
        overwrite: false,
    })
}

#[tokio::test]
async fn cancel_job_and_follow_events() {
    let llm = mock_llm::start_slow(Duration::from_millis(800)).await;
    let home = tempfile::tempdir().unwrap();
    let server = animesubs_server::start(ServerConfig::local(home.path(), 0))
        .await
        .unwrap();
    let client = Client::new(server.url(), server.token()).unwrap();
    let write = |name: &str| {
        let path = home.path().join(name);
        std::fs::write(&path, mock_llm::SRT).unwrap();
        path
    };

    let first = client
        .submit(&spec(write("a.srt"), &llm.uri()))
        .await
        .unwrap();
    let second = client
        .submit(&spec(write("b.srt"), &llm.uri()))
        .await
        .unwrap();
    let mut events = client.events(Some(&first.id)).await.unwrap();
    let snapshot = events.next().await.unwrap().unwrap();
    assert_eq!(snapshot.id, first.id);

    // Queued: cancelled at once, and cancelling again changes nothing.
    let cancelled = client.cancel_job(&second.id).await.unwrap();
    assert_eq!(cancelled.state, JobState::Cancelled);
    assert_eq!(client.cancel_job(&second.id).await.unwrap(), cancelled);

    // Follow the first job to its terminal frame.
    let mut last = snapshot;
    while !last.state.is_terminal() {
        last = events.next().await.unwrap().expect("stream ended early");
        assert_eq!(last.id, first.id);
    }
    assert_eq!(last.state, JobState::Succeeded, "{:?}", last.error);

    match client.cancel_job(&first.id).await {
        Err(ClientError::Api { status, error }) => {
            assert_eq!((status, error.code.as_str()), (409, "not_cancellable"))
        }
        other => panic!("expected 409, got {other:?}"),
    }
    match client.events(Some("job-nope")).await {
        Err(ClientError::Api { status, .. }) => assert_eq!(status, 404),
        other => panic!("expected 404, got {other:?}"),
    }

    // The stream ends when the server stops.
    let mut all = client.events(None).await.unwrap();
    assert_eq!(all.next().await.unwrap().unwrap().id, first.id);
    assert_eq!(all.next().await.unwrap().unwrap().id, second.id);
    server.stop().await.unwrap();
    assert!(all.next().await.unwrap().is_none());
}
