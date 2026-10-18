use super::{
    execution::create_tabs,
    wire::{McpSession, assert_rejected, entries},
};
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_no_profile_applies_to_all_created_tabs() {
    #[cfg(windows)]
    let (shell, command) = ("powershell", "Write-Output $env:FUNCTERM_LOAD_PROFILE");
    #[cfg(unix)]
    let (shell, command) = ("bash", "printf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"");
    for (arguments, expected) in [(&[][..], "1"), (&["--no-profile"][..], "0")] {
        let mut session = McpSession::with_arguments(arguments);
        let creation = session.call(
            "new_tab",
            json ! ({ "exec" : [{ "starting_shell" : shell } , { "starting_shell" : shell }] }),
        );
        for entry in entries(&creation) {
            let tab_id = entry
                .pointer("/result/tab_id")
                .and_then(Value::as_str)
                .unwrap();
            let response = session . call ("send_command" , json ! ({ "exec" : [{ "tab_id" : tab_id , "command" : command }] , "wait_timeout" : 10_u64 }) ,) ;
            let result = entries(&response).first().unwrap();
            assert_eq!(
                result.pointer("/result/command/exit_code"),
                Some(&json!(0_i32)),
                "{response}"
            );
            assert_eq!(
                result
                    .pointer("/result/command/stdout")
                    .and_then(Value::as_str)
                    .unwrap()
                    .trim(),
                expected,
                "{response}"
            );
        }
        drop(session);
    }
}

#[test]
fn mcp_output_policy_removes_and_rejects_the_choice_independently_of_privileges() {
    for privilege in [None, Some("normal"), Some("elevated")] {
        for mode in [None, Some("separated"), Some("terminal")] {
            let mut arguments = vec!["--no-profile"];
            if let Some(value) = privilege {
                arguments.extend(["--privilege-mode", value]);
            }
            if let Some(value) = mode {
                arguments.extend(["--output-mode", value]);
            }
            let mut session = McpSession::with_arguments(&arguments);
            let response = session.request("tools/list", json!({}));
            let tools = response
                .pointer("/result/tools")
                .and_then(Value::as_array)
                .unwrap();
            assert_eq!(tools.len(), 4, "{response}");
            let tool = tools
                .iter()
                .find(|tool| tool.get("name") == Some(&json!("send_command")))
                .unwrap();
            let choice =
                tool.pointer("/inputSchema/properties/exec/items/properties/separate_output");
            assert_eq!(choice.is_some(), mode.is_none(), "{response}");
            if mode.is_some() {
                for separate in [false, true] {
                    let rejected = session.call("send_command", json!({
                        "exec": [{"tab_id": "unused", "command": "echo UNUSED", "separate_output": separate}],
                        "wait_timeout": 0_u64
                    }));
                    assert_rejected(&rejected);
                }
            }
            drop(session);
        }
    }
}

#[test]
fn mcp_fixed_output_mode_is_used_and_command_views_keep_the_same_shape() {
    for (mode, field, excluded) in [
        ("separated", "stdout", "output"),
        ("terminal", "output", "stdout"),
    ] {
        let mut session = McpSession::with_arguments(&["--no-profile", "--output-mode", mode]);
        let [tab, _other] = create_tabs(&mut session);
        let response = session.call(
            "send_command",
            json!({
                "exec": [{"tab_id": tab, "command": "echo FIXED_OUTPUT"}],
                "wait_timeout": 10_u64
            }),
        );
        let result = entries(&response)
            .first()
            .unwrap()
            .pointer("/result/command")
            .unwrap();
        assert!(
            result
                .get(field)
                .and_then(Value::as_str)
                .unwrap()
                .contains("FIXED_OUTPUT"),
            "{response}"
        );
        assert!(result.get(excluded).is_none(), "{response}");
        assert_eq!(result.get("exit_code"), Some(&json!(0_i32)));
        let id = result.get("command_id").unwrap().as_str().unwrap();
        let viewed = session.call("view", json!({"ids": [id], "wait_timeout": 0_u64}));
        drop(session);
        let saved = entries(&viewed)
            .first()
            .unwrap()
            .pointer("/result/command")
            .unwrap();
        assert_eq!(saved.get(field), result.get(field), "{viewed}");
        assert!(saved.get(excluded).is_none(), "{viewed}");
    }
}

#[test]
fn mcp_selects_output_per_batch_entry_and_defaults_to_separation() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let response = session.call(
        "send_command",
        json!({
            "exec": [
                {"tab_id": first, "command": "echo SEPARATED_ENTRY", "separate_output": true},
                {"tab_id": second, "command": "echo TERMINAL_ENTRY", "separate_output": false}
            ],
            "wait_timeout": 10_u64
        }),
    );
    let results = entries(&response);
    assert!(
        results
            .first()
            .unwrap()
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap()
            .contains("SEPARATED_ENTRY"),
        "{response}"
    );
    assert!(
        results
            .get(1)
            .unwrap_or_else(|| panic!("missing terminal entry: {response}"))
            .pointer("/result/command/output")
            .and_then(Value::as_str)
            .unwrap()
            .contains("TERMINAL_ENTRY"),
        "{response}"
    );
    let default = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": second, "command": "echo DEFAULT_SEPARATED"}],
            "wait_timeout": 10_u64
        }),
    );
    drop(session);
    assert!(
        entries(&default)
            .first()
            .unwrap()
            .pointer("/result/command/stdout")
            .and_then(Value::as_str)
            .unwrap()
            .contains("DEFAULT_SEPARATED"),
        "{default}"
    );
}
