extern crate alloc;
use super::wire::{McpSession, assert_field, assert_rejected, field, text};
use crate::support::locked_with_env;
use alloc::rc::Rc;
use rmcp::serde_json::{Value, json};
#[test]
fn schema_and_rejections_cover_every_privilege_and_output_policy_combination() {
    let daemon = Rc::new(locked_with_env(&[]));
    for privilege in [None, Some("normal"), Some("elevated")] {
        for output in [None, Some("separated"), Some("terminal")] {
            let mut arguments = vec!["--no-profile"];
            for (option, value) in [("--privilege-mode", privilege), ("--output-mode", output)] {
                if let Some(mode) = value {
                    arguments.extend([option, mode]);
                }
            }
            let mut session = McpSession::on(&daemon, &arguments);
            let response = session.request("tools/list", json!({}));
            let tools = response
                .pointer("/result/tools")
                .and_then(Value::as_array)
                .unwrap();
            assert_eq!(tools.len(), 4, "{response}");
            for tool in tools {
                assert_references(tool);
            }
            for (name, property, fixed) in [
                ("new_tab", "elevated", privilege.is_some()),
                ("send_command", "separate_output", output.is_some()),
            ] {
                let tool = tools
                    .iter()
                    .find(|tool| text(tool, "/name") == name)
                    .unwrap();
                let path = format!("/inputSchema/properties/exec/items/properties/{property}");
                let choice = tool.pointer(&path);
                assert_eq!(choice.is_none(), fixed, "{response}");
                if let Some(schema) = choice {
                    if name == "new_tab" {
                        assert_field(schema, "/type", "boolean");
                        assert_field(schema, "/default", false);
                        let required = field(tool, "/inputSchema/properties/exec/items/required");
                        assert!(required.as_array().unwrap().contains(&json!(property)));
                    } else {
                        assert_field(schema, "/type", json!(["boolean", "null"]));
                    }
                } else {
                    for value in [false, true] {
                        let request = if name == "new_tab" {
                            json!({"exec": [{"starting_shell": super::execution::SHELL, "elevated": value}]})
                        } else {
                            json!({"exec": [{"tab_id": "unused", "command": "echo UNUSED", "separate_output": value}], "wait_timeout": 0_u64})
                        };
                        assert_rejected(&session.call(name, request));
                    }
                }
            }
        }
    }
}
fn assert_references(value: &Value) {
    if let Some(object) = value.as_object() {
        assert!(!object.contains_key("$ref") || object.len() == 1, "{value}");
        for child in object.values() {
            assert_references(child);
        }
    } else if let Some(array) = value.as_array() {
        for child in array {
            assert_references(child);
        }
    }
}
#[test]
fn wait_timeout_limits_apply_to_schemas_and_calls_before_the_multiplier() {
    let daemon = Rc::new(locked_with_env(&[]));
    for version in ["2025-06-18", "2025-11-25"] {
        let mut session = McpSession::with_protocol_version(
            Rc::clone(&daemon),
            &[
                "--wait-timeout-min=0.25",
                "--wait-timeout-max=0.5",
                "--wait-timeout-multiplier=2",
                "--output-mode=terminal",
            ],
            version,
        );
        let response = session.request("tools/list", json!({}));
        for tool in field(&response, "/result/tools").as_array().unwrap() {
            if text(tool, "/name") != "new_tab" {
                let timeout = field(tool, "/inputSchema/properties/wait_timeout");
                assert_field(timeout, "/minimum", 0.25_f64);
                assert_field(timeout, "/maximum", 0.5_f64);
            }
        }
        for wait in [0.24_f64, 0.25_f64, 0.5_f64, 0.51_f64] {
            for (name, arguments) in [
                (
                    "view",
                    json!({"ids": ["missing"], "wait_timeout": wait, "start_token": 1_u64}),
                ),
                (
                    "manual_write",
                    json!({"exec": [{"tab_id": "missing", "text": "", "bytes": null}], "wait_timeout": wait}),
                ),
                (
                    "send_command",
                    json!({"exec": [{"tab_id": "missing", "command": "echo UNUSED"}], "wait_timeout": wait}),
                ),
            ] {
                let called = session.call(name, arguments);
                assert_rejected(&called);
                assert_eq!(
                    called
                        .pointer("/result/structuredContent/results")
                        .is_some(),
                    (0.25_f64..=0.5_f64).contains(&wait),
                    "{called}"
                );
            }
        }
    }
}
