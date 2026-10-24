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
    let daemon = Rc::new(locked_with_env(&[]));
    for version in ["2025-06-18", "2025-11-25"] {
        let mut http = HttpSession::new(&daemon, version);
        let mut stdio = McpSession::with_protocol_version(Rc::clone(&daemon), version);
        let http_tools = http.request("tools/list", json!({}));
        let stdio_tools = stdio.request("tools/list", json!({}));
        assert_eq!(
            field(&http_tools, "/result/tools"),
            field(&stdio_tools, "/result/tools")
        );
        assert_eq!(
            field(&http_tools, "/result/tools")
                .as_array()
                .unwrap()
                .len(),
            4
        );
        let created = http.call("new_tab", json!({
            "exec": [{"starting_shell": super::execution::SHELL, "starting_directory": temp_root()}]
        }));
        let tab = text(super::wire::result(&created), "/tab_id");
        let executed = stdio.execute(tab, "echo SHARED_TRANSPORT_STATE", true, 10.0);
        let id = text(command_result(&executed), "/command_id");
        let observed = http.call("view", json!({"ids": [id], "wait_timeout": 10_u64}));
        assert!(
            text(command_result(&observed), "/stdout").contains("SHARED_TRANSPORT_STATE"),
            "{observed}"
        );
        let reverse = http.call("send_command", json!({
            "exec": [{"tab_id": tab, "command": "echo DIRECT_DAEMON_EXECUTION", "separate_output": true}], "wait_timeout": 10_u64
        }));
        assert!(
            text(command_result(&reverse), "/stdout").contains("DIRECT_DAEMON_EXECUTION"),
            "{reverse}"
        );
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
                assert_field(&response, "/error/code", -32_602_i32);
            } else {
                assert_field(&response, "/result/isError", true);
                assert!(response.get("error").is_none(), "{response}");
            }
        }
        for response in [
            http.call("missing_tool", json!({})),
            stdio.call("missing_tool", json!({})),
        ] {
            assert_field(&response, "/error/code", -32_602_i32);
        }
        drop(stdio);
        assert_field(&http.request("ping", json!({})), "/result", json!({}));
    }
}
