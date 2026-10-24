use crate::support::TestGuard;
use core::time::Duration;
use reqwest::{
    Method, StatusCode,
    blocking::{Client, RequestBuilder, Response},
};
use rmcp::serde_json::{Value, json};
pub(super) struct HttpSession {
    client: Client,
    endpoint: String,
    version: String,
    session_id: String,
    sequence: u64,
}
impl HttpSession {
    pub(super) fn new(daemon: &TestGuard, version: &str) -> Self {
        let client = Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(40))
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap();
        let endpoint = format!("http://{}/mcp", daemon.http_address());
        let response = client
            .post(&endpoint)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .body(
                json!({
                    "jsonrpc": "2.0", "id": 0_u64, "method": "initialize",
                    "params": {
                        "protocolVersion": version, "capabilities": {},
                        "clientInfo": {"name": "http-contract-tests", "version": "1"}
                    }
                })
                .to_string(),
            )
            .send()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let session_id = response
            .headers()
            .get("Mcp-Session-Id")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned();
        let initialized = decode(response, 0);
        assert_eq!(
            initialized.pointer("/result/protocolVersion"),
            Some(&json!(version))
        );
        assert!(initialized.pointer("/result/capabilities/tools").is_some());
        let session = Self {
            client,
            endpoint,
            version: version.to_owned(),
            session_id,
            sequence: 0,
        };
        let notification = session
            .builder(Method::POST)
            .body(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}).to_string())
            .send()
            .unwrap();
        assert_eq!(notification.status(), StatusCode::ACCEPTED);
        assert!(notification.bytes().unwrap().is_empty());
        session
    }
    pub(super) fn builder(&self, method: Method) -> RequestBuilder {
        self.client
            .request(method, &self.endpoint)
            .header("Accept", "application/json, text/event-stream")
            .header("Content-Type", "application/json")
            .header("Mcp-Session-Id", &self.session_id)
            .header("Mcp-Protocol-Version", &self.version)
    }
    pub(super) fn request(&mut self, method: &str, params: Value) -> Value {
        self.sequence += 1;
        let mut message = json!({
            "jsonrpc": "2.0", "id": self.sequence, "method": method,
        });
        message
            .as_object_mut()
            .unwrap()
            .insert("params".to_owned(), params);
        let response = self
            .builder(Method::POST)
            .body(message.to_string())
            .send()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        decode(response, self.sequence)
    }
    pub(super) fn call(&mut self, name: &str, arguments: Value) -> Value {
        let mut params = json!({"name": name});
        params
            .as_object_mut()
            .unwrap()
            .insert("arguments".to_owned(), arguments);
        self.request("tools/call", params)
    }
}
fn decode(response: Response, id: u64) -> Value {
    let event_stream = response
        .headers()
        .get("Content-Type")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("text/event-stream");
    let text = response.text().unwrap();
    if !event_stream {
        let message: Value = rmcp::serde_json::from_str(&text).unwrap();
        assert_eq!(message.get("id"), Some(&json!(id)));
        return message;
    }
    text.lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .filter(|data| !data.trim().is_empty())
        .map(|data| rmcp::serde_json::from_str::<Value>(data).unwrap())
        .find(|message| message.get("id") == Some(&json!(id)))
        .unwrap_or_else(|| panic!("missing JSON-RPC response {id} in SSE stream: {text}"))
}
