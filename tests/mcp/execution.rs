use super::wire::{McpSession, entries};
use core::time::Duration;
use rmcp::serde_json::{Value, json};
use std::time::Instant;
#[cfg(windows)]
const SHELL: &str = "powershell";
#[cfg(not(windows))]
const SHELL: &str = "bash";
pub(super) fn create_tabs(session: &mut McpSession) -> [String; 2] {
    let response = session . call ("new_tab" , json ! ({ "exec" : [{ "starting_shell" : SHELL } , { "starting_shell" : SHELL , "starting_directory" : crate :: support :: temp_root () }] })) ;
    let created = entries(&response);
    assert_eq!(created.len(), 2);
    core::array::from_fn(|index| {
        created
            .get(index)
            .unwrap_or_else(|| panic!("tab index should exist"))
            .pointer("/result/tab_id")
            .and_then(Value::as_str)
            .unwrap()
            .to_owned()
    })
}
#[test]
fn mcp_new_tab_failure_does_not_hide_successful_creation() {
    let mut session = McpSession::new();
    let response = session . call ("new_tab" , json ! ({ "exec" : [{ "starting_shell" : SHELL , "starting_directory" : env ! ("CARGO_BIN_EXE_functerm") } , { "starting_shell" : SHELL }] })) ;
    let results = entries(&response);
    assert_eq!(results.len(), 2);
    assert!(results.first().unwrap().get("error").is_some());
    assert!(
        results
            .last()
            .unwrap()
            .pointer("/result/tab_id")
            .and_then(Value::as_str)
            .is_some()
    );
}
#[test]
fn mcp_batch_preserves_order_and_isolates_errors() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    assert_ne!(first, second);
    let response = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : first , "command" : "echo FIRST_BATCH_RESULT" } , { "tab_id" : "missing" , "command" : "echo UNUSED" } , { "tab_id" : second , "command" : "echo LAST_BATCH_RESULT" }] , "wait_timeout" : 5_u64 })) ;
    let results = entries(&response);
    assert_eq!(results.len(), 3);
    let content = response
        .pointer("/result/content")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(content.len(), 3);
    assert!(content.iter().all(|block| {
        !block
            .get("text")
            .and_then(Value::as_str)
            .unwrap()
            .starts_with("exec[")
    }));
    assert!(
        results
            .first()
            .unwrap()
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap()
            .contains("FIRST_BATCH_RESULT")
    );
    assert!(
        results
            .get(1)
            .unwrap_or_else(|| panic!("middle result should exist"))
            .get("error")
            .is_some()
    );
    assert!(
        results
            .last()
            .unwrap()
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap()
            .contains("LAST_BATCH_RESULT")
    );
    let viewed = session.call(
        "view",
        json ! ({ "ids" : [first , second] , "wait_timeout" : 0_u64 }),
    );
    assert!(
        entries(&viewed)
            .iter()
            .all(|entry| entry.pointer("/result/screen").is_some())
    );
    let written = session . call ("manual_write" , json ! ({ "exec" : [{ "tab_id" : first , "text" : "x" , "bytes" : [120_u8] } , { "tab_id" : second }] , "wait_timeout" : 0_u64 })) ;
    let errors = entries(&written);
    assert!(
        errors
            .first()
            .unwrap()
            .get("error")
            .and_then(Value::as_str)
            .unwrap()
            .contains("cannot be provided together")
    );
    assert!(
        errors
            .last()
            .unwrap()
            .get("error")
            .and_then(Value::as_str)
            .unwrap()
            .contains("either text or bytes")
    );
}
#[test]
fn mcp_wait_budget_is_shared_and_timeout_does_not_cancel_commands() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    #[cfg(windows)]
    let command = "Start-Sleep -Seconds 4\nWrite-Output BATCH_FINISHED";
    #[cfg(not(windows))]
    let command = "sleep 4\necho BATCH_FINISHED";
    let started = Instant::now();
    let response = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : first , "command" : command } , { "tab_id" : second , "command" : command }] , "wait_timeout" : 1.5_f64 })) ;
    assert!(
        started.elapsed() < Duration::from_millis(2800),
        "{response}"
    );
    let mut ids = Vec::new();
    for entry in entries(&response) {
        assert_eq!(
            entry.pointer("/result/command/finished"),
            Some(&json!(false))
        );
        let id = entry
            .pointer("/result/command/command_id")
            .and_then(Value::as_str)
            .unwrap();
        ids.push(id);
    }
    let completed = session.call("view", json ! ({ "ids" : ids , "wait_timeout" : 8_u64 }));
    for entry in entries(&completed) {
        assert_eq!(
            entry.pointer("/result/command/finished"),
            Some(&json!(true))
        );
        assert!(
            entry
                .pointer("/result/command/stdout")
                .and_then(Value::as_str)
                .unwrap()
                .contains("BATCH_FINISHED")
        );
    }
}
