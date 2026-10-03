use crate::support::{TestGuard, locked_with_env};
use core::time::Duration;
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
    _daemon: TestGuard,
}
impl McpSession {
    pub(super) fn new() -> Self {
        Self::with_arguments(&[])
    }
    pub(super) fn with_arguments(arguments: &[&str]) -> Self {
        let daemon = locked_with_env(&[]);
        let mut child = Command::new(env!("CARGO_BIN_EXE_functerm"))
            .arg("mcp")
            .args(arguments)
            .envs(daemon.env())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
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
        let initialized = session . request ("initialize" , json ! ({ "protocolVersion" : "2024-11-05" , "capabilities" : { } , "clientInfo" : { "name" : "batch-contract-tests" , "version" : "1" } }) ,) ;
        assert!(initialized.get("result").is_some(), "{initialized}");
        session.send(&json ! ({ "jsonrpc" : "2.0" , "method" : "notifications/initialized" }));
        session
    }
    pub(super) fn request(&mut self, method: &str, params: Value) -> Value {
        self.sequence += 1;
        let mut message =
            json ! ({ "jsonrpc" : "2.0" , "id" : self . sequence , "method" : method });
        message
            .as_object_mut()
            .unwrap()
            .insert("params".to_owned(), params);
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
        let mut params = json ! ({ "name" : name });
        params
            .as_object_mut()
            .unwrap()
            .insert("arguments".to_owned(), arguments);
        self.request("tools/call", params)
    }
    fn send(&mut self, message: &Value) {
        writeln!(self.input, "{message}").unwrap();
        self.input.flush().unwrap();
    }
}
impl Drop for McpSession {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            self.child.kill().unwrap();
        }
        self.child.wait().unwrap();
    }
}
pub(super) fn entries(response: &Value) -> &Vec<Value> {
    response
        .pointer("/result/structuredContent/results")
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("missing batch results: {response}"))
}
pub(super) fn assert_rejected(response: &Value) {
    assert!(
        response.get("error").is_some()
            || response.pointer("/result/isError") == Some(&Value::Bool(true)),
        "{response}"
    );
}
