extern crate alloc;
use super::execution::create_tabs;
use super::wire::{McpSession, assert_field, entries, text};
use crate::support::{TestGuard, locked_with_env};
use alloc::rc::Rc;
use rmcp::serde_json::json;
const PROBE: &str = "FUNCTERM_MCP_ENV_PROBE";
const DAEMON_ONLY: &str = "FUNCTERM_DAEMON_ONLY_PROBE";
#[test]
fn mcp_tabs_inherit_each_parent_environment_on_shared_daemon() {
    let daemon = Rc::new(locked_with_env(&[
        (PROBE, "daemon environment"),
        (DAEMON_ONLY, "daemon only"),
    ]));
    for expected in [Some("first MCP = 中文"), Some("second MCP = 独立"), None] {
        let mut client = client_session(&daemon, expected);
        let tabs = create_tabs::<2>(&mut client);
        assert_environment(&mut client, &tabs, expected.unwrap_or(""));
    }
}
#[test]
fn tab_environment_changes_do_not_change_new_mcp_tabs() {
    let daemon = Rc::new(locked_with_env(&[]));
    let mut client = client_session(&daemon, Some("MCP parent"));
    let [changed, sibling] = create_tabs(&mut client);
    #[cfg(windows)]
    let mutation = "$env:FUNCTERM_MCP_ENV_PROBE = 'tab only'";
    #[cfg(not(windows))]
    let mutation = "export FUNCTERM_MCP_ENV_PROBE='tab only'";
    let response = client.execute(&changed, mutation, false, 5.0);
    assert_field(super::wire::command_result(&response), "/exit_code", 0_i32);
    assert_environment(&mut client, &[changed], "tab only");
    assert_environment(&mut client, &[sibling], "MCP parent");
    let later = create_tabs::<1>(&mut client);
    assert_environment(&mut client, &later, "MCP parent");
}
fn client_session(daemon: &Rc<TestGuard>, value: Option<&str>) -> McpSession {
    let mut environment = daemon
        .env()
        .into_iter()
        .filter(|pair| pair.0 != PROBE && pair.0 != DAEMON_ONLY)
        .collect::<Vec<_>>();
    if let Some(probe) = value {
        environment.push((PROBE.to_owned(), probe.to_owned()));
    }
    McpSession::with_context(Rc::clone(daemon), &["--no-profile"], &environment)
}
fn assert_environment(client: &mut McpSession, tabs: &[String], expected: &str) {
    #[cfg(windows)]
    let probe = "if (Test-Path Env:FUNCTERM_DAEMON_ONLY_PROBE) { throw 'daemon environment leaked' }\nWrite-Output $env:FUNCTERM_MCP_ENV_PROBE";
    #[cfg(not(windows))]
    let probe = "if [[ -v FUNCTERM_DAEMON_ONLY_PROBE ]]; then\n    exit 2\nfi\nprintf '%s\\n' \"${FUNCTERM_MCP_ENV_PROBE-}\"";
    let operations = tabs
        .iter()
        .map(|tab| json!({"tab_id": tab, "command": probe, "separate_output": true}))
        .collect::<Vec<_>>();
    let response = client.call(
        "send_command",
        json!({"exec": operations, "wait_timeout": 5_u64}),
    );
    assert_eq!(entries(&response).len(), tabs.len(), "{response}");
    for entry in entries(&response) {
        assert_field(entry, "/result/command/finished", true);
        assert_field(entry, "/result/command/exit_code", 0_i32);
        assert_eq!(
            text(entry, "/result/command/stdout").trim_end_matches(['\r', '\n']),
            expected,
            "{response}"
        );
    }
}
