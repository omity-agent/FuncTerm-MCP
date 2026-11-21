extern crate alloc;
use super::{
    http_peer::HttpSession,
    wire::{McpSession, assert_field, command_result, field, text},
};
use crate::support::{locked_with_env, temp_root};
use alloc::rc::Rc;
use rmcp::serde_json::json;
#[test]
fn both_versions_share_tools_terminal_state_and_errors_across_transports() {
    let daemon = Rc::new(locked_with_env(&[("FUNCTERM_ZSH", "")]));
    for version in ["2025-06-18", "2025-11-25"] {
        let mut http = HttpSession::new(&daemon, version);
        let mut stdio =
            McpSession::with_protocol_version(Rc::clone(&daemon), &["--no-profile"], version);
        let http_tools = http.request("tools/list", json!({}));
        let stdio_tools = stdio.request("tools/list", json!({}));
        assert_eq!(
            field(&http_tools, "/result/tools"),
            field(&stdio_tools, "/result/tools")
        );
        let created = http.call("new_tab", json!({
            "tabs": [{"starting_shell": super::execution::SHELL, "starting_directory": temp_root()}]
        }));
        let tab = text(super::wire::result(&created), "/tab_id");
        let executed = stdio.execute(tab, "echo SHARED_TRANSPORT_STATE", true, 10.0);
        let id = text(command_result(&executed), "/command_id");
        let observed = http.call(
            "view",
            json!({"queries": [{"id": id}], "wait_timeout": 10_u64}),
        );
        assert!(
            text(command_result(&observed), "/stdout").contains("SHARED_TRANSPORT_STATE"),
            "{observed}"
        );
        let reverse = http.call("run", json!({
            "commands": [{"tab_id": tab, "command": "echo DIRECT_DAEMON_EXECUTION", "separate_output": true}], "wait_timeout": 10_u64
        }));
        assert!(
            text(command_result(&reverse), "/stdout").contains("DIRECT_DAEMON_EXECUTION"),
            "{reverse}"
        );
        for response in [
            http.call(
                "run",
                json!({"commands": [{"tab_id": true}], "wait_timeout": 0_u64}),
            ),
            stdio.call(
                "run",
                json!({"commands": [{"tab_id": true}], "wait_timeout": 0_u64}),
            ),
        ] {
            if version == "2025-06-18" {
                assert_field(&response, "/error/code", -32_602_i32);
            } else {
                assert_field(&response, "/result/isError", true);
                assert!(response.get("error").is_none(), "{response}");
            }
        }
        for (name, params, code) in [
            ("missing_tool", json!({}), -32_602_i32),
            (
                "new_tab",
                json!({"tabs": [{"starting_shell": "zsh"}]}),
                -32_603_i32,
            ),
        ] {
            for response in [http.call(name, params.clone()), stdio.call(name, params)] {
                assert_field(&response, "/error/code", code);
                assert!(response.get("result").is_none(), "{response}");
                if code == -32_603_i32 {
                    assert_field(
                        &response,
                        "/error/message",
                        "FuncTerm could not complete the tool call",
                    );
                }
            }
        }
        drop(stdio);
        assert_field(&http.request("ping", json!({})), "/result", json!({}));
    }
}
