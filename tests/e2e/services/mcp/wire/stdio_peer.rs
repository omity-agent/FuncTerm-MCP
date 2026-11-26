extern crate alloc;
#[path = "replies.rs"]
mod replies;
use crate::support::{ChildGuard, TestGuard, locked_with_env};
use alloc::rc::Rc;
use core::time::Duration;
pub(super) use replies::{
    assert_field, assert_finished_output, assert_rejected, command_result, completed_commands,
    entries, field, result, text, text_documents,
};
use rmcp::serde_json::{Value, json};
use std::{
    io::{BufRead as _, BufReader, Write as _},
    process::{ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};
pub(super) struct McpSession {
    _child: ChildGuard,
    input: ChildStdin,
    responses: Receiver<Value>,
    sequence: u64,
    _daemon: Rc<TestGuard>,
}
impl McpSession {
    pub(super) fn new() -> Self {
        let daemon = Rc::new(locked_with_env(&[]));
        Self::on(&daemon, &["--no-profile", "--wait-timeout-min=0"])
    }
    pub(super) fn on(daemon: &Rc<TestGuard>, arguments: &[&str]) -> Self {
        Self::with_protocol_version(Rc::clone(daemon), arguments, "2025-11-25")
    }
    pub(super) fn with_protocol_version(
        daemon: Rc<TestGuard>,
        arguments: &[&str],
        version: &str,
    ) -> Self {
        let mut command = Command::new(crate::support::exe());
        command
            .arg("mcp")
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        crate::support::apply_test_environment(&mut command, &daemon.env());
        let mut child = ChildGuard::spawn(&mut command).unwrap();
        let input = child.child_mut().stdin.take().unwrap();
        let stdout = child.child_mut().stdout.take().unwrap();
        let (sender, responses) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let message = rmcp::serde_json::from_str(&line.unwrap()).unwrap();
                if sender.send(message).is_err() {
                    break;
                }
            }
        });
        let mut session = Self {
            _child: child,
            input,
            responses,
            sequence: 0,
            _daemon: daemon,
        };
        let initialized = session.request("initialize", initialize_params(version));
        assert_field(&initialized, "/result/protocolVersion", version);
        session.send(&initialized_notification());
        session
    }
    pub(super) fn request(&mut self, method: &str, params: Value) -> Value {
        self.sequence += 1;
        let message = request_message(method, params, self.sequence);
        self.send(&message);
        loop {
            let response = self
                .responses
                .recv_timeout(Duration::from_secs(40))
                .unwrap();
            if response.get("id") == Some(&json!(self.sequence)) {
                return response;
            }
        }
    }
    pub(super) fn call(&mut self, name: &str, arguments: Value) -> Value {
        self.request("tools/call", tool_arguments(name, arguments))
    }
    pub(super) fn execute(&mut self, tab: &str, source: &str, separate: bool, wait: f64) -> Value {
        self.execute_many(&[tab], source, separate, wait)
    }
    pub(super) fn execute_many(
        &mut self,
        tabs: &[&str],
        source: &str,
        separate: bool,
        wait: f64,
    ) -> Value {
        let commands = tabs
            .iter()
            .map(|tab| json!({"tab_id": tab, "command": source, "separate_output": separate}))
            .collect::<Vec<_>>();
        self.call("run", json!({"commands": commands, "wait_timeout": wait}))
    }
    pub(super) fn view(&mut self, queries: &[(&str, usize)], wait: f64) -> Value {
        let entries = queries
            .iter()
            .map(|&(id, start)| json!({"id": id, "start_token": start}))
            .collect::<Vec<_>>();
        self.call("view", json!({"queries": entries, "wait_timeout": wait}))
    }
    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }
}
pub(super) fn request_message(method: &str, params: Value, sequence: u64) -> Value {
    with_payload(
        json!({"jsonrpc": "2.0", "id": sequence, "method": method}),
        "params",
        params,
    )
}
pub(super) fn tool_arguments(name: &str, arguments: Value) -> Value {
    with_payload(json!({"name": name}), "arguments", arguments)
}
fn with_payload(mut envelope: Value, key: &str, value: Value) -> Value {
    envelope
        .as_object_mut()
        .unwrap()
        .insert(key.to_owned(), value);
    envelope
}
pub(super) fn initialized_notification() -> Value {
    json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
}
pub(super) fn initialize_params(version: &str) -> Value {
    json!({
        "protocolVersion": version,
        "capabilities": {},
        "clientInfo": {"name": "contract-tests", "version": "1"}
    })
}
