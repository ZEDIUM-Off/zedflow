mod common;

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use common::http_capture::{CapturedRequest, FixtureResponse, HttpCapture};
use common::live_credentials::{self, CredentialSource, LiveCredentialConfig};
use common::oauth_fixture::{
    DeviceCodePollingFixture, OAuthPoll, device_authorization_pending_body,
    openai_codex_access_token,
};
use common::sse_fixture::{SseFixture, parse_sse};
use common::ws_fixture::{WebSocketFixture, WsEvent};
use serde_json::json;

#[test]
fn http_capture_sequences_requests_and_redacts_secret_headers() {
    let capture = HttpCapture::new([
        FixtureResponse::json(200, &json!({ "ok": true })),
        FixtureResponse::text(202, "accepted"),
    ]);

    let first = CapturedRequest::new("POST", "https://example.test/v1")
        .header("Authorization", "Bearer sk-secret")
        .json_body(&json!({ "hello": "world" }));
    let debug = format!("{first:?}");
    assert!(debug.contains("<redacted>"));
    assert!(!debug.contains("sk-secret"));

    let response = capture.request(first).unwrap();
    assert_eq!(response.status, 200);
    assert_eq!(response.body_json(), Some(json!({ "ok": true })));

    let response = capture
        .request(CapturedRequest::new("GET", "https://example.test/next"))
        .unwrap();
    assert_eq!(response.status, 202);
    assert_eq!(response.body_text(), "accepted");
    assert_eq!(capture.requests().len(), 2);
    capture.assert_no_pending_responses().unwrap();
}

#[test]
fn sse_fixture_builds_and_parses_pi_style_frames() {
    let body = SseFixture::new()
        .json("message_start", &json!({ "type": "message_start" }))
        .data("[DONE]")
        .to_string();

    let frames = parse_sse(&body);
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].event.as_deref(), Some("message_start"));
    assert_eq!(
        frames[0].data_text(),
        json!({ "type": "message_start" }).to_string()
    );
    assert_eq!(frames[1].data_text(), "[DONE]");
}

#[test]
fn websocket_fixture_captures_headers_sent_frames_and_server_events() {
    let fixture = WebSocketFixture::with_events([
        WsEvent::Open,
        WsEvent::json(&json!({ "type": "response.completed" })),
    ]);

    fixture.connect(
        "wss://chatgpt.com/backend-api/codex/responses",
        [
            ("Authorization", "Bearer codex-secret"),
            ("session-id", "s1"),
        ],
    );
    fixture.send_json(&json!({ "prompt": "hi" }));

    let connection = fixture.connection().unwrap();
    assert_eq!(connection.headers.get("session-id"), Some(&"s1".to_owned()));
    assert_eq!(
        connection.redacted_headers().get("authorization"),
        Some(&"<redacted>".to_owned())
    );
    assert_eq!(fixture.sent_json(), [json!({ "prompt": "hi" })]);
    assert_eq!(fixture.next_event(), Some(WsEvent::Open));
    assert_eq!(fixture.pending_event_count(), 1);
}

#[test]
fn oauth_fixture_matches_pi_poll_timing_and_token_shapes() {
    let mut polling = DeviceCodePollingFixture::new(2, 900, 1_778_284_800_000)
        .responses([OAuthPoll::slow_down(), OAuthPoll::Complete("token")]);

    assert_eq!(polling.poll_until_complete().unwrap(), "token");
    assert_eq!(
        polling.poll_times_ms(),
        &[1_778_284_800_000, 1_778_284_807_000]
    );
    assert_eq!(
        device_authorization_pending_body()["error"]["code"],
        "deviceauth_authorization_pending"
    );
    assert!(openai_codex_access_token("account-123").contains("signature"));
}

#[test]
fn live_credentials_detects_pi_auth_entries_without_logging_values() {
    let dir = std::env::temp_dir().join(format!(
        "zedflow-ai-live-credentials-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let auth_path = dir.join("auth.json");
    fs::write(
        &auth_path,
        r#"{"openai-codex":{"type":"oauth","access":"codex-secret","refresh":"r","expires":9999999999999}}"#,
    )
    .unwrap();

    let config = LiveCredentialConfig {
        auth_path: auth_path.clone(),
    };
    let codex = live_credentials::capability_with_config("openai-codex", &config);
    assert!(codex.available);
    assert_eq!(codex.source, Some(CredentialSource::PiAuthJsonOAuth));

    let openrouter = live_credentials::capability_with_config("openrouter", &config);
    let skip = openrouter.skip_message().unwrap();
    assert!(skip.contains("openrouter"));
    assert!(skip.contains("OPENROUTER_API_KEY"));
    assert!(!skip.contains("codex-secret"));

    fs::remove_dir_all(dir).unwrap();
}
