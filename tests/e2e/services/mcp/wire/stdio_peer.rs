extern crate alloc;
#[path = "replies.rs"]
mod replies;
use crate::support::{TestGuard, locked_with_env};
use alloc::rc::Rc;
use core::time::Duration;
pub(super) use replies::{
    assert_field, assert_rejected, assert_saved, command_result, entries, field, result, text,
    text_documents,
};
use rmcp::serde_json::{Value, json};
use std::{
    io::{BufRead as _, BufReader, Write as _},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
};
pub(super) struct McpSession {
    child: Child,
    input: ChildStdin,
    responses: Receiver<Value>,
    sequence: u64,
    _daemon: Rc<TestGuard>,
}
impl McpSession {
    pub(super) fn new() -> Self {
        let daemon = Rc::new(locked_with_env(&[]));
        Self::on(&daemon, &["--no-profile"])
    }
    pub(super) fn on(daemon: &Rc<TestGuard>, arguments: &[&str]) -> Self {
        let environment = daemon.env();
        Self::start(Rc::clone(daemon), arguments, &environment, "2025-11-25")
    }
    pub(super) fn with_protocol_version(daemon: Rc<TestGuard>, version: &str) -> Self {
        let environment = daemon.env();
        Self::start(daemon, &["--no-profile"], &environment, version)
    }
    fn start(
        daemon: Rc<TestGuard>,
        arguments: &[&str],
        environment: &[(String, String)],
        version: &str,
    ) -> Self {
        let mut command = Command::new(crate::support::exe());
        command
            .arg("mcp")
            .args(arguments)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());
        crate::support::apply_test_environment(&mut command, environment);
        let mut child = command.spawn().unwrap();
        let input = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
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
            child,
            input,
            responses,
            sequence: 0,
            _daemon: daemon,
        };
        let initialized = session.request(
            "initialize",
            json!({
                "protocolVersion": version,
                "capabilities": {},
                "clientInfo": {"name": "batch-contract-tests", "version": "1"}
            }),
        );
        assert!(initialized.get("result").is_some(), "{initialized}");
        assert_eq!(
            initialized.pointer("/result/protocolVersion"),
            Some(&json!(version))
        );
        session.send(&json ! ({ "jsonrpc" : "2.0" , "method" : "notifications/initialized" }));
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
        let exec = tabs
            .iter()
            .map(|tab| json!({"tab_id": tab, "command": source, "separate_output": separate}))
            .collect::<Vec<_>>();
        self.call("send_command", json!({"exec": exec, "wait_timeout": wait}))
    }
    pub(super) fn view(&mut self, ids: &[&str], start: usize, wait: f64) -> Value {
        self.call(
            "view",
            json!({"ids": ids, "start_token": start, "wait_timeout": wait}),
        )
    }
    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }
}
pub(super) fn request_message(method: &str, params: Value, sequence: u64) -> Value {
    let mut message = json!({"jsonrpc": "2.0", "id": sequence, "method": method, "params": null});
    *message.get_mut("params").unwrap() = params;
    message
}
pub(super) fn tool_arguments(name: &str, arguments: Value) -> Value {
    let mut parameters = json!({"name": name, "arguments": null});
    *parameters.get_mut("arguments").unwrap() = arguments;
    parameters
}
impl Drop for McpSession {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
        }
        self.child.wait().unwrap();
    }
}
