extern crate alloc;
use super::wire::{McpSession, assert_field, assert_rejected, text};
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
                        let required = tool
                            .pointer("/inputSchema/properties/exec/items/required")
                            .and_then(Value::as_array)
                            .unwrap();
                        assert!(required.contains(&json!(property)));
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
