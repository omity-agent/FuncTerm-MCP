#[cfg(test)]
#[path = "mcp/presentation/boundaries.rs"]
mod boundaries;
#[cfg(test)]
#[path = "mcp/presentation/cli_cursor.rs"]
mod cli_cursor;
#[cfg(test)]
#[path = "mcp/wire/coexistence.rs"]
mod coexistence;
#[cfg(test)]
#[path = "mcp/execution.rs"]
mod execution;
#[path = "mcp/wire/http_peer.rs"]
mod http_peer;
#[cfg(test)]
#[path = "mcp/inheritance.rs"]
mod inheritance;
#[cfg(test)]
#[path = "mcp/input_delivery.rs"]
mod input_delivery;
#[cfg(test)]
#[path = "mcp/presentation/input_schema.rs"]
mod input_schema;
#[cfg(test)]
#[path = "mcp/inspection.rs"]
mod inspection;
#[cfg(test)]
#[path = "mcp/wire/lifetime.rs"]
mod lifetime;
#[cfg(test)]
#[path = "mcp/presentation/live_output.rs"]
mod live_output;
#[cfg(test)]
#[path = "mcp/presentation/pagination.rs"]
mod pagination;
#[cfg(test)]
#[path = "mcp/profiles.rs"]
mod profiles;
#[cfg(test)]
#[path = "mcp/presentation/selection.rs"]
mod selection;
#[cfg(test)]
#[path = "mcp/tool_restrictions.rs"]
mod tool_restrictions;
#[path = "mcp/wire/stdio_peer.rs"]
mod wire;
use rmcp::serde_json::{Value, json};
use wire::{McpSession, assert_rejected, entries};
#[test]
fn mcp_rejects_invalid_batches_before_dispatch() {
    let mut session = McpSession::new();
    let directory = crate::support::temp_dir("rejected-dispatch");
    let creation = session . call ("new_tab" , json ! ({ "exec" : [{ "starting_shell" : execution :: SHELL , "starting_directory" : env ! ("CARGO_BIN_EXE_functerm") } , { "starting_shell" : execution :: SHELL , "starting_directory" : directory }] })) ;
    assert!(
        entries(&creation).first().unwrap().get("error").is_some(),
        "{creation}"
    );
    let tab = entries(&creation)
        .last()
        .unwrap()
        .pointer("/result/tab_id")
        .and_then(Value::as_str)
        .unwrap();
    for (name, arguments) in [
        ("new_tab", json ! ({ "exec" : [] })),
        (
            "send_command",
            json ! ({ "exec" : [{ "tab_id" : tab , "command" : "echo UNEXPECTED > rejected.txt" } , { "tab_id" : tab , "command" : "echo UNEXPECTED > rejected.txt" }] , "wait_timeout" : 0_u64 }),
        ),
        (
            "view",
            json ! ({ "ids" : [tab] , "wait_timeout" : - 1_i64 }),
        ),
        (
            "view",
            json!({"ids": [tab], "wait_timeout": 0_u64, "start_token": -1_i64}),
        ),
        (
            "view",
            json!({"ids": [tab], "wait_timeout": 0_u64, "start_token": 0_u64}),
        ),
        (
            "view",
            json!({"ids": [tab], "wait_timeout": 0_u64, "start_token": 1.5_f64}),
        ),
        (
            "view",
            json!({"ids": [tab], "wait_timeout": 0_u64, "start_token": null}),
        ),
    ] {
        let response = session.call(name, arguments);
        assert_rejected(&response);
        assert!(
            response
                .pointer("/result/structuredContent/results")
                .is_none(),
            "{response}"
        );
    }
    let recovered = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab, "command": "echo DISPATCH_RECOVERED", "separate_output": true}],
            "wait_timeout": 5_u64
        }),
    );
    assert!(
        entries(&recovered)
            .first()
            .unwrap()
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap()
            .contains("DISPATCH_RECOVERED"),
        "{recovered}"
    );
    assert!(!directory.join("rejected.txt").exists());
    drop(session);
}
