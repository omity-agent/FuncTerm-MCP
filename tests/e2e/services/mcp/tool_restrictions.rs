use super::wire::{McpSession, assert_rejected, entries};
use crate::permission_policy::{QUERY, SHELL, daemon_privileged, privileged_output};
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_schema_only_exposes_elevated_when_permissions_are_selectable() {
    for mode in [None, Some("normal"), Some("elevated")] {
        let arguments = mode.map_or_else(
            || vec!["--no-profile"],
            |value| vec!["--no-profile", "--privilege-mode", value],
        );
        let mut session = McpSession::with_arguments(&arguments);
        let response = session.request("tools/list", json!({}));
        let tools = response
            .pointer("/result/tools")
            .and_then(Value::as_array)
            .unwrap();
        assert_eq!(tools.len(), 4, "{response}");
        let tool = tools
            .iter()
            .find(|tool| tool.get("name") == Some(&json!("new_tab")))
            .unwrap();
        let property = tool.pointer("/inputSchema/properties/exec/items/properties/elevated");
        if mode.is_none() {
            assert_eq!(property.unwrap().get("type"), Some(&json!("boolean")));
            assert_eq!(property.unwrap().get("default"), Some(&json!(false)));
            let required = tool
                .pointer("/inputSchema/properties/exec/items/required")
                .and_then(Value::as_array)
                .unwrap_or_else(|| panic!("New Tab schema has no required fields: {response}"));
            assert!(!required.contains(&json!("elevated")));
        } else {
            assert!(property.is_none(), "{response}");
            for elevated in [false, true] {
                let rejected = session.call(
                    "new_tab",
                    json ! ({ "exec" : [{ "starting_shell" : SHELL , "elevated" : elevated }] }),
                );
                assert_rejected(&rejected);
            }
        }
        drop(session);
    }
}
#[test]
fn mcp_fixed_permissions_are_used_for_new_tabs() {
    for (mode, elevated) in [("normal", false), ("elevated", true)] {
        let mut session = McpSession::with_arguments(&["--no-profile", "--privilege-mode", mode]);
        let creation = session.call(
            "new_tab",
            json ! ({ "exec" : [{ "starting_shell" : SHELL }] }),
        );
        if elevated && !daemon_privileged() {
            let error = entries(&creation)
                .first()
                .unwrap()
                .get("error")
                .and_then(Value::as_str)
                .unwrap();
            assert!(error.contains("requires the Daemon"), "{creation}");
        } else {
            assert_created_permission(&mut session, &creation, elevated);
        }
        drop(session);
    }
}
#[test]
fn mcp_new_tab_selects_permissions_per_entry() {
    let mut session = McpSession::new();
    let creation = session . call ("new_tab" , json ! ({ "exec" : [{ "starting_shell" : SHELL } , { "starting_shell" : SHELL , "elevated" : false } , { "starting_shell" : SHELL , "elevated" : true }] })) ;
    for (index, entry) in entries(&creation).iter().enumerate() {
        let elevated = index == 2;
        if elevated && !daemon_privileged() {
            assert!(
                entry
                    .get("error")
                    .and_then(Value::as_str)
                    .unwrap()
                    .contains("requires the Daemon")
            );
        } else {
            let tab_id = entry
                .pointer("/result/tab_id")
                .and_then(Value::as_str)
                .unwrap_or_else(|| panic!("Tab creation failed: {creation}"));
            assert_permission(&mut session, tab_id, elevated);
        }
    }
    drop(session);
}
fn assert_created_permission(session: &mut McpSession, creation: &Value, elevated: bool) {
    let tab_id = entries(creation)
        .first()
        .unwrap()
        .pointer("/result/tab_id")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("Tab creation failed: {creation}"));
    assert_permission(session, tab_id, elevated);
}
fn assert_permission(session: &mut McpSession, tab_id: &str, elevated: bool) {
    let response = session.call(
        "send_command",
        json ! ({ "exec" : [{ "tab_id" : tab_id , "command" : QUERY }] , "wait_timeout" : 10_u64 }),
    );
    let result = entries(&response).first().unwrap();
    assert_eq!(
        result.pointer("/result/command/exit_code"),
        Some(&json!(0_i32)),
        "{response}"
    );
    let output = result
        .pointer("/result/command/stdout")
        .and_then(Value::as_str)
        .unwrap();
    assert_eq!(privileged_output(output), elevated, "{response}");
}
