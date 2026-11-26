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
#[path = "mcp/presentation/threshold_notes.rs"]
mod threshold_notes;
#[path = "mcp/wire/stdio_peer.rs"]
mod wire;
use itertools::Itertools as _;
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
    let tabs: Vec<_> = [crate::support::exe(), directory.clone()]
        .iter()
        .map(|cwd| json!({"starting_shell": execution::SHELL, "starting_directory": cwd}))
        .collect();
    let creation = session.call("new_tab", json!({"tabs": tabs}));
    let (invalid, valid) = entries(&creation)
        .iter()
        .collect_tuple()
        .unwrap_or_else(|| panic!("unexpected batch: {creation}"));
    assert!(invalid.get("error").is_some(), "{creation}");
    let tab = text(valid, "/result/tab_id");
    let duplicate = json!({"tab_id": tab, "command": "echo UNEXPECTED > rejected.txt"});
    let batches = [
        ("new_tab", json!({"tabs": []})),
        ("run", json!({"commands": [], "wait_timeout": 0_u64})),
        ("manual_write", json!({"inputs": [], "wait_timeout": 0_u64})),
        ("view", json!({"queries": [], "wait_timeout": 0_u64})),
        (
            "run",
            json!({"commands": [duplicate, duplicate], "wait_timeout": 0_u64}),
        ),
        (
            "view",
            json!({"queries": [{"id": tab}], "wait_timeout": -1_i64}),
        ),
    ];
    let offsets = [json!(-1_i64), json!(0_u64), json!(1.5_f64), json!(null)].map(|offset| {
        (
            "view",
            json!({"queries": [{"id": tab, "start_token": offset}], "wait_timeout": 0_u64}),
        )
    });
    for (name, arguments) in batches.into_iter().chain(offsets) {
        let response = session.call(name, arguments);
        assert_rejected(&response);
        assert!(
            response
                .pointer("/result/structuredContent/results")
                .is_none(),
            "{response}"
        );
    }
    let recovered = session.execute(tab, "echo DISPATCH_RECOVERED", true, 5.0);
    assert!(text(command_result(&recovered), "/stdout").contains("DISPATCH_RECOVERED"));
    assert!(!directory.join("rejected.txt").exists());
}
