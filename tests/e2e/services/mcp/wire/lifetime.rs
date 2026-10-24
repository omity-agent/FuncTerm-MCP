use super::http_peer::HttpSession;
use crate::support::{locked_with_env, run_cli_with_env};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use reqwest::{Method, StatusCode};
use rmcp::serde_json::json;
#[test]
fn http_sessions_support_get_delete_and_reinitialization_for_both_versions() {
    let daemon = locked_with_env(&[]);
    for version in ["2025-06-18", "2025-11-25"] {
        let session = HttpSession::new(&daemon, version);
        let stream = session
            .builder(Method::GET)
            .header("Accept", "text/event-stream")
            .send()
            .unwrap();
        assert_eq!(stream.status(), StatusCode::OK);
        assert!(
            stream
                .headers()
                .get("Content-Type")
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("text/event-stream")
        );
        drop(stream);
        let deleted = session.builder(Method::DELETE).send().unwrap();
        assert_eq!(deleted.status(), StatusCode::ACCEPTED);
        let expired = session
            .builder(Method::POST)
            .body(json!({"jsonrpc": "2.0", "id": 5_u64, "method": "ping"}).to_string())
            .send()
            .unwrap();
        assert_eq!(expired.status(), StatusCode::NOT_FOUND);
        let mut recovered = HttpSession::new(&daemon, version);
        assert_eq!(
            recovered.request("ping", json!({})).get("result"),
            Some(&json!({}))
        );
    }
    drop(daemon);
}
#[test]
fn invalid_origin_and_protocol_headers_are_rejected_without_breaking_the_session() {
    let daemon = locked_with_env(&[]);
    let mut session = HttpSession::new(&daemon, "2025-11-25");
    let forbidden = session
        .builder(Method::GET)
        .header("Origin", "https://untrusted.example:443")
        .send()
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
    let invalid_version = session
        .builder(Method::GET)
        .headers(HeaderMap::from_iter([(
            HeaderName::from_static("mcp-protocol-version"),
            HeaderValue::from_static("invalid-version"),
        )]))
        .send()
        .unwrap();
    assert_eq!(invalid_version.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        session.request("ping", json!({})).get("result"),
        Some(&json!({}))
    );
    drop(daemon);
}
#[test]
fn daemon_shutdown_closes_http_streams_and_releases_its_listener() {
    let daemon = locked_with_env(&[]);
    let session = HttpSession::new(&daemon, "2025-11-25");
    let stream = session
        .builder(Method::GET)
        .header("Accept", "text/event-stream")
        .send()
        .unwrap();
    assert_eq!(stream.status(), StatusCode::OK);
    let stopped = run_cli_with_env(&["daemon", "stop"], &daemon.env());
    assert!(stopped.status.success(), "{stopped:?}");
    stream.text().unwrap();
    session.builder(Method::GET).send().unwrap_err();
    drop(daemon);
}
