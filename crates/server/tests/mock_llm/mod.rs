//! An OpenAI-compatible endpoint that "translates" by prefixing each numbered
//! line with `PT: `, so tests exercise the real engine and provider code
//! without network access or a model.

#![allow(dead_code)]

use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

struct PrefixTranslation;

impl Respond for PrefixTranslation {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: serde_json::Value = serde_json::from_slice(&request.body).unwrap();
        let user = body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["role"] == "user")
            .and_then(|message| message["content"].as_str())
            .unwrap()
            .to_string();
        let lines: Vec<String> = user
            .lines()
            .filter_map(|line| {
                let (id, text) = line.strip_prefix('<')?.split_once("> ")?;
                id.parse::<u64>().ok()?;
                Some(format!("<{id}> PT: {text}"))
            })
            .collect();
        ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "choices": [{
                "message": { "content": lines.join("\n") },
                "finish_reason": "stop"
            }]
        }))
    }
}

/// Start the mock; use `server.uri()` as the provider `base_url`.
pub async fn start() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(PrefixTranslation)
        .mount(&server)
        .await;
    server
}

/// A mock that rejects every request with HTTP 401.
pub async fn start_rejecting() -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(401).set_body_string("invalid api key"))
        .mount(&server)
        .await;
    server
}

pub const SRT: &str = "1\n00:00:01,000 --> 00:00:02,000\nHello there\n\n\
                       2\n00:00:03,000 --> 00:00:04,000\n<i>See you</i> tomorrow\n";
