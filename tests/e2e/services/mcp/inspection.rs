use super::{
    execution::create_tabs,
    wire::{McpSession, entries},
};
use rmcp::serde_json::{Value, json};
use sugar_path::SugarPath as _;
#[test]
fn mcp_view_accepts_mixed_ids_and_preserves_order_and_errors() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let command = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : first , "command" : "echo VIEW_ID_LIST_RESULT" }] , "wait_timeout" : 5_u64 }) ,) ;
    let command_id = entries(&command)
        .first()
        .unwrap()
        .pointer("/result/command/command_id")
        .and_then(Value::as_str)
        .unwrap();
    let first_cwd = entries(&command)
        .first()
        .unwrap()
        .pointer("/result/shell/cwd")
        .and_then(Value::as_str)
        .unwrap();
    let second_cwd = crate::support::temp_root()
        .normalize()
        .to_slash()
        .into_owned();
    assert_ne!(first_cwd, second_cwd);
    let viewed = session . call ("view" , json ! ({ "ids" : [second , command_id , "missing" , first , second] , "wait_timeout" : 0_u64 }) ,) ;
    let results = entries(&viewed);
    assert_eq!(results.len(), 5);
    assert_eq!(viewed.pointer("/result/isError"), Some(&json!(true)));
    for (index, cwd) in [
        (0, second_cwd.as_str()),
        (3, first_cwd),
        (4, second_cwd.as_str()),
    ] {
        let result = results
            .get(index)
            .unwrap_or_else(|| panic!("tab result should exist"));
        assert_eq!(
            result.pointer("/result/shell/cwd").and_then(Value::as_str),
            Some(cwd)
        );
        assert!(result.pointer("/result/screen").is_some());
    }
    let completed = results
        .get(1)
        .unwrap_or_else(|| panic!("command result should exist"));
    assert_eq!(
        completed.pointer("/result/command/finished"),
        Some(&json!(true))
    );
    assert!(
        completed
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap()
            .contains("VIEW_ID_LIST_RESULT")
    );
    let failure = results
        .get(2)
        .unwrap_or_else(|| panic!("error result should exist"));
    assert!(
        failure
            .get("error")
            .and_then(Value::as_str)
            .unwrap()
            .contains("unknown id missing")
    );
}
