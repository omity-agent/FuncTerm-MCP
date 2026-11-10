#[path = "mcp/wire/coexistence.rs"]
mod coexistence;
#[path = "mcp/execution.rs"]
mod execution;
#[path = "mcp/wire/http_peer.rs"]
mod http_peer;
#[path = "mcp/input_delivery.rs"]
mod input_delivery;
#[path = "mcp/inspection.rs"]
mod inspection;
#[path = "mcp/presentation/live_output.rs"]
mod live_output;
#[path = "mcp/presentation/pagination.rs"]
mod pagination;
#[path = "mcp/presentation/policy_matrix.rs"]
mod policy_matrix;
#[path = "mcp/wire/stdio_peer.rs"]
mod wire;
use rmcp::serde_json::json;
use wire::{McpSession, assert_rejected, command_result, entries, text};
#[test]
fn mcp_rejects_invalid_batches_before_dispatch() {
    for (option, values) in [
        ("multiplier", ["0", "-0.5", "NaN", "inf", "-inf"]),
        ("min", ["-0.5", "NaN", "inf", "-inf", "1e99"]),
        ("max", ["-0.5", "NaN", "inf", "-inf", "1e99"]),
    ] {
        for value in values {
            let rejected =
                crate::support::run_cli(&["mcp", &format!("--wait-timeout-{option}={value}")]);
            assert!(!rejected.status.success(), "{rejected:?}");
        }
    }
    let mut session = McpSession::new();
    let directory = crate::support::temp_dir("rejected-dispatch");
    let creation = session . call ("new_tab" , json ! ({ "tabs" : [{ "starting_shell" : execution :: SHELL , "starting_directory" : env ! ("CARGO_BIN_EXE_functerm") } , { "starting_shell" : execution :: SHELL , "starting_directory" : directory }] })) ;
    assert!(
        entries(&creation).first().unwrap().get("error").is_some(),
        "{creation}"
    );
    let tab = text(entries(&creation).last().unwrap(), "/result/tab_id");
    for (name, arguments) in [
        ("new_tab", json!({"tabs": []})),
        (
            "send_command",
            json!({"commands": [], "wait_timeout": 0_u64}),
        ),
        ("manual_write", json!({"inputs": [], "wait_timeout": 0_u64})),
        ("view", json!({"queries": [], "wait_timeout": 0_u64})),
        (
            "send_command",
            json ! ({ "commands" : [{ "tab_id" : tab , "command" : "echo UNEXPECTED > rejected.txt" } , { "tab_id" : tab , "command" : "echo UNEXPECTED > rejected.txt" }] , "wait_timeout" : 0_u64 }),
        ),
        (
            "view",
            json!({"queries": [{"id": tab}], "wait_timeout": -1_i64}),
        ),
        (
            "view",
            json!({"queries": [{"id": tab, "start_token": -1_i64}], "wait_timeout": 0_u64}),
        ),
        (
            "view",
            json!({"queries": [{"id": tab, "start_token": 0_u64}], "wait_timeout": 0_u64}),
        ),
        (
            "view",
            json!({"queries": [{"id": tab, "start_token": 1.5_f64}], "wait_timeout": 0_u64}),
        ),
        (
            "view",
            json!({"queries": [{"id": tab, "start_token": null}], "wait_timeout": 0_u64}),
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
            "commands": [{"tab_id": tab, "command": "echo DISPATCH_RECOVERED", "separate_output": true}],
            "wait_timeout": 5_u64
        }),
    );
    assert!(text(command_result(&recovered), "/stdout").contains("DISPATCH_RECOVERED"));
    assert!(!directory.join("rejected.txt").exists());
    drop(session);
}
