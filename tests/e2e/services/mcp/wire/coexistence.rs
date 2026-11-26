extern crate alloc;
use super::{
    http_peer::HttpSession,
    wire::{McpSession, assert_field, command_result, completed_commands, entries, field, text},
};
use crate::support::{locked_with_env, temp_root};
use alloc::rc::Rc;
use core::fmt::Write as _;
use rmcp::serde_json::{Value, json};
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
#[test]
fn single_parameter_run_matches_across_transports() {
    let daemon = Rc::new(locked_with_env(&[("FUNCTERM_SHELL_LOAD_PROFILE", "false")]));
    let mut environment = daemon.env();
    environment
        .iter_mut()
        .find(|setting| setting.0 == "FUNCTERM_MCP_HTTP_BIND")
        .unwrap()
        .1 = daemon.http_address().to_owned();
    let restarted = crate::support::run_cli_with_env(
        &["daemon", "restart", "--single-parameter-run"],
        &environment,
    );
    assert!(restarted.status.success(), "{restarted:?}");
    let version = "2025-06-18";
    let mut http = HttpSession::new(&daemon, version);
    let mut stdio =
        McpSession::with_protocol_version(Rc::clone(&daemon), &["--single-parameter-run"], version);
    let http_tools = http.request("tools/list", json!({}));
    let stdio_tools = stdio.request("tools/list", json!({}));
    let tools = field(&http_tools, "/result/tools");
    assert_eq!(tools, field(&stdio_tools, "/result/tools"));
    let run = tools
        .as_array()
        .unwrap()
        .iter()
        .find(|tool| text(tool, "/name") == "run")
        .unwrap();
    assert_eq!(
        field(run, "/inputSchema/properties")
            .as_object()
            .unwrap()
            .len(),
        1
    );
    assert_field(run, "/inputSchema/required", json!(["script"]));
    assert_field(run, "/inputSchema/properties/script/type", "string");
    let blocks = [
        (super::execution::SHELL, "echo SHELL_RESULT", "SHELL_RESULT"),
        (
            "bun",
            "console.log(`\n*** output: BUN_RESULT\n`)",
            "*** output: BUN_RESULT",
        ),
        #[cfg(windows)]
        ("cmd", "echo CMD_RESULT", "CMD_RESULT"),
    ];
    let tabs = blocks.map(|(shell, ..)| json!({"starting_shell": shell}));
    let created = http.call("new_tab", json!({"tabs": tabs}));
    let created_tabs = entries(&created);
    assert_eq!(created_tabs.len(), blocks.len(), "{created}");
    let mut script = String::new();
    for (entry, (_, source, _)) in created_tabs.iter().zip(blocks) {
        let tab = text(entry, "/result/tab_id");
        writeln!(script, "*** tab_id: {tab}; wait_timeout: 10;\n{source}").unwrap();
    }
    let markers = blocks.map(|(_, _, marker)| marker);
    assert_script_results(&script, &markers, |name, arguments| {
        http.call(name, arguments)
    });
    assert_script_results(&script, &markers, |name, arguments| {
        stdio.call(name, arguments)
    });
}
fn assert_script_results(
    script: &str,
    markers: &[&str],
    mut call: impl FnMut(&str, Value) -> Value,
) {
    let response = call("run", json!({"script": script}));
    let commands = entries(&response);
    assert_eq!(commands.len(), markers.len(), "{response}");
    let ids: Vec<_> = commands
        .iter()
        .map(|entry| text(entry, "/result/command/command_id"))
        .collect();
    let completed = completed_commands(&ids, |arguments| call("view", arguments));
    for (entry, &marker) in entries(&completed).iter().zip(markers) {
        assert_field(entry, "/result/command/exit_code", 0_i32);
        assert!(
            text(entry, "/result/command/output").contains(marker),
            "{entry}"
        );
    }
}
