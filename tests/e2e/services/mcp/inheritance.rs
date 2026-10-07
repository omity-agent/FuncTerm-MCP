extern crate alloc;
use super::execution::create_tabs;
use super::wire::{McpSession, entries};
use crate::support::{TestGuard, locked_with_env};
use alloc::rc::Rc;
use rmcp::serde_json::{Value, json};
const PROBE: &str = "FUNCTERM_MCP_ENV_PROBE";
const DAEMON_ONLY: &str = "FUNCTERM_DAEMON_ONLY_PROBE";
#[test]
fn mcp_tabs_inherit_each_parent_environment_on_shared_daemon() {
    let daemon = Rc::new(locked_with_env(&[
        (PROBE, "daemon environment"),
        (DAEMON_ONLY, "daemon only"),
    ]));
    let clients = [
        (
            client_session(&daemon, Some("first MCP = 中文")),
            "first MCP = 中文",
        ),
        (
            client_session(&daemon, Some("second MCP = 独立")),
            "second MCP = 独立",
        ),
        (client_session(&daemon, None), ""),
    ];
    drop(daemon);
    for (mut client, expected) in clients {
        let tabs = create_tabs(&mut client);
        assert_environment(&mut client, &tabs, expected);
        let later = create_tabs(&mut client);
        assert_environment(&mut client, &later, expected);
        drop(client);
    }
}
#[test]
fn tab_environment_changes_do_not_change_new_mcp_tabs() {
    let daemon = Rc::new(locked_with_env(&[]));
    let mut client = client_session(&daemon, Some("MCP parent"));
    drop(daemon);
    let [changed, sibling] = create_tabs(&mut client);
    #[cfg(windows)]
    let mutation = "$env:FUNCTERM_MCP_ENV_PROBE = 'tab only'";
    #[cfg(not(windows))]
    let mutation = "export FUNCTERM_MCP_ENV_PROBE='tab only'";
    let response = client . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : changed , "command" : mutation }] , "wait_timeout" : 5_u64 , }) ,) ;
    assert_eq!(
        entries(&response)
            .first()
            .unwrap()
            .pointer("/result/command/exit_code"),
        Some(&json!(0_i32)),
        "{response}",
    );
    assert_environment(&mut client, &[changed], "tab only");
    assert_environment(&mut client, &[sibling], "MCP parent");
    let later = create_tabs(&mut client);
    assert_environment(&mut client, &later, "MCP parent");
    drop(client);
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
    let operations = tabs
        .iter()
        .map(|tab| json ! ({ "tab_id" : tab , "command" : probe_command () }))
        .collect::<Vec<_>>();
    let response = client.call(
        "send_command",
        json ! ({ "exec" : operations , "wait_timeout" : 5_u64 }),
    );
    let results = entries(&response);
    assert_eq!(results.len(), tabs.len(), "{response}");
    for entry in results {
        assert_eq!(
            entry.pointer("/result/command/finished"),
            Some(&json!(true)),
            "{response}"
        );
        assert_eq!(
            entry.pointer("/result/command/exit_code"),
            Some(&json!(0_i32)),
            "{response}"
        );
        let stdout = entry
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap();
        assert_eq!(
            stdout.trim_end_matches(['\r', '\n']),
            expected,
            "{response}"
        );
    }
}
#[cfg(windows)]
const fn probe_command() -> &'static str {
    "if (Test-Path Env:FUNCTERM_DAEMON_ONLY_PROBE) { throw 'daemon environment leaked' }\nWrite-Output $env:FUNCTERM_MCP_ENV_PROBE"
}
#[cfg(not(windows))]
const fn probe_command() -> &'static str {
    "if [[ -v FUNCTERM_DAEMON_ONLY_PROBE ]]; then\n    exit 2\nfi\nprintf '%s\\n' \"${FUNCTERM_MCP_ENV_PROBE-}\""
}
