extern crate alloc;
use super::{
    execution::create_tabs,
    wire::{McpSession, assert_field, assert_saved, command_result, entries, text},
};
use crate::support::locked_with_env;
use alloc::rc::Rc;
use rmcp::serde_json::json;
#[test]
fn mcp_fixed_output_mode_is_used_and_command_views_keep_the_same_shape() {
    let daemon = Rc::new(locked_with_env(&[]));
    for (mode, field, excluded) in [
        ("separated", "stdout", "output"),
        ("terminal", "output", "stdout"),
    ] {
        let mut session = McpSession::on(&daemon, &["--no-profile", "--output-mode", mode]);
        let [tab] = create_tabs(&mut session);
        let response = session.call(
            "send_command",
            json!({
                "exec": [{"tab_id": tab, "command": "echo FIXED_OUTPUT"}], "wait_timeout": 10_u64
            }),
        );
        let command = command_result(&response);
        assert!(
            text(command, &format!("/{field}")).contains("FIXED_OUTPUT"),
            "{response}"
        );
        assert!(command.get(excluded).is_none(), "{response}");
        assert_field(command, "/exit_code", 0_i32);
        let id = text(command, "/command_id");
        let viewed = session.view(&[id], 1, 0.0);
        assert_saved(&response, &viewed);
    }
}
#[test]
fn mcp_selects_output_per_batch_entry_and_uses_the_configured_default() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let response = session.call(
        "send_command",
        json!({
            "exec": [
                {"tab_id": first, "command": "echo SEPARATED_ENTRY", "separate_output": true},
                {"tab_id": second, "command": "echo TERMINAL_ENTRY", "separate_output": false}
            ], "wait_timeout": 10_u64
        }),
    );
    let results = entries(&response);
    assert!(
        text(results.first().unwrap(), "/result/command/stdout").contains("SEPARATED_ENTRY"),
        "{response}"
    );
    assert!(
        text(
            results
                .get(1)
                .unwrap_or_else(|| panic!("missing terminal entry: {response}")),
            "/result/command/output"
        )
        .contains("TERMINAL_ENTRY"),
        "{response}"
    );
    let default = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": second, "command": "echo CONFIGURED_DEFAULT"}], "wait_timeout": 10_u64
        }),
    );
    let settings: toml::Value = toml::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/settings.toml"
    )))
    .unwrap();
    let separate = settings
        .get("command_separate_output")
        .and_then(toml::Value::as_bool)
        .unwrap();
    let (field, excluded) = if separate {
        ("stdout", "output")
    } else {
        ("output", "stdout")
    };
    let command = command_result(&default);
    assert!(
        text(command, &format!("/{field}")).contains("CONFIGURED_DEFAULT"),
        "{default}"
    );
    assert!(command.get(excluded).is_none(), "{default}");
}
