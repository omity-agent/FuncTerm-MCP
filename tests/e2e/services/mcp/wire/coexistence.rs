extern crate alloc;
use super::{
    http_peer::HttpSession,
    wire::{McpSession, entries},
};
use crate::support::{locked_with_env, temp_root};
use alloc::rc::Rc;
use rmcp::serde_json::{Value, json};

#[cfg(windows)]
const SHELL: &str = "powershell";
#[cfg(unix)]
const SHELL: &str = "bash";

#[test]
fn both_versions_share_tools_and_terminal_state_across_transports() {
    let daemon = Rc::new(locked_with_env(&[]));
    for version in ["2025-06-18", "2025-11-25"] {
        let mut http = HttpSession::new(&daemon, version);
        let mut stdio = McpSession::with_protocol_version(Rc::clone(&daemon), version);
        let http_tools = http.request("tools/list", json!({}));
        let stdio_tools = stdio.request("tools/list", json!({}));
        assert_eq!(
            http_tools.pointer("/result/tools"),
            stdio_tools.pointer("/result/tools")
        );
        assert_eq!(
            http_tools
                .pointer("/result/tools")
                .and_then(Value::as_array)
                .unwrap()
                .len(),
            4
        );
        let created = http.call(
            "new_tab",
            json!({
                "exec": [{"starting_shell": SHELL, "starting_directory": temp_root()}]
            }),
        );
        let tab_id = entries(&created)
            .first()
            .unwrap()
            .pointer("/result/tab_id")
            .and_then(Value::as_str)
            .unwrap();
        let executed = stdio.call(
            "send_command",
            json!({
                "exec": [{
                    "tab_id": tab_id, "command": "echo SHARED_TRANSPORT_STATE",
                    "separate_output": true
                }],
                "wait_timeout": 10_u64
            }),
        );
        drop(stdio);
        let command_id = entries(&executed)
            .first()
            .unwrap()
            .pointer("/result/command/command_id")
            .and_then(Value::as_str)
            .unwrap();
        let observed = http.call("view", json!({"ids": [command_id], "wait_timeout": 10_u64}));
        assert!(
            entries(&observed)
                .first()
                .unwrap()
                .pointer("/result/command/stdout")
                .and_then(Value::as_str)
                .unwrap()
                .contains("SHARED_TRANSPORT_STATE"),
            "{observed}"
        );
        let reverse = http.call("send_command", json!({
            "exec": [{"tab_id": tab_id, "command": "echo DIRECT_DAEMON_EXECUTION", "separate_output": true}],
            "wait_timeout": 10_u64
        }));
        assert!(
            entries(&reverse)
                .first()
                .unwrap()
                .pointer("/result/command/stdout")
                .and_then(Value::as_str)
                .unwrap()
                .contains("DIRECT_DAEMON_EXECUTION"),
            "{reverse}"
        );
    }
    drop(daemon);
}

#[test]
fn input_errors_follow_the_negotiated_protocol_on_both_transports() {
    let daemon = Rc::new(locked_with_env(&[]));
    for version in ["2025-06-18", "2025-11-25"] {
        let mut http = HttpSession::new(&daemon, version);
        let mut stdio = McpSession::with_protocol_version(Rc::clone(&daemon), version);
        for response in [
            http.call(
                "send_command",
                json!({"exec": [{"tab_id": true}], "wait_timeout": 0_u64}),
            ),
            stdio.call(
                "send_command",
                json!({"exec": [{"tab_id": true}], "wait_timeout": 0_u64}),
            ),
        ] {
            if version == "2025-06-18" {
                assert_eq!(
                    response.pointer("/error/code"),
                    Some(&json!(-32_602_i32)),
                    "{response}"
                );
            } else {
                assert_eq!(
                    response.pointer("/result/isError"),
                    Some(&json!(true)),
                    "{response}"
                );
                assert!(response.get("error").is_none(), "{response}");
            }
        }
        for response in [
            http.call("missing_tool", json!({})),
            stdio.call("missing_tool", json!({})),
        ] {
            assert_eq!(
                response.pointer("/error/code"),
                Some(&json!(-32_602_i32)),
                "{response}"
            );
        }
        drop(stdio);
        assert_eq!(
            http.request("ping", json!({})).get("result"),
            Some(&json!({}))
        );
    }
    drop(daemon);
}
