use super::{
    execution::create_tabs,
    pagination::{command_result, expected_suffix},
    wire::{McpSession, entries},
};
use rmcp::serde_json::{Value, json};
#[test]
fn mcp_separated_output_counts_both_streams_and_rejects_skipping_tokens() {
    let mut session = McpSession::new();
    let [tab, _other] = create_tabs(&mut session);
    #[cfg(windows)]
    let command =
        "Write-Output 'hello world'\nWrite-Error -Message '你好 🧠' -ErrorAction Continue";
    #[cfg(not(windows))]
    let command = "printf 'hello world'\nprintf '你好 🧠' >&2";
    let accepted = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab, "command": command, "separate_output": true}],
            "wait_timeout": 5_u64
        }),
    );
    let result = command_result(&accepted);
    let stdout = result.get("stdout").and_then(Value::as_str).unwrap();
    let stderr = result.get("stderr").and_then(Value::as_str).unwrap();
    assert_eq!(stdout.trim_end(), "hello world");
    assert!(stderr.contains("你好 🧠"), "{accepted}");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let total = tokenizer.count(stdout.as_bytes()) + tokenizer.count(stderr.as_bytes());
    assert_eq!(result.get("tokens"), Some(&json!(total)));
    let id = result.get("command_id").and_then(Value::as_str).unwrap();
    let rejected = session.call(
        "view",
        json!({"ids": [id, tab], "wait_timeout": 0_u64, "start_token": 2_u64}),
    );
    let results = entries(&rejected);
    assert!(
        results
            .first()
            .unwrap()
            .get("error")
            .and_then(Value::as_str)
            .unwrap()
            .contains("start_token must be 1")
    );
    assert!(results.last().unwrap().pointer("/result/screen").is_some());
    let unchanged = session.call(
        "view",
        json!({"ids": [id], "wait_timeout": 0_u64, "start_token": 1_u64}),
    );
    let full = command_result(&unchanged);
    for field in ["stdout", "stderr", "tokens", "exit_code", "finished"] {
        assert_eq!(full.get(field), result.get(field), "{unchanged}");
    }
    drop(session);
}
#[test]
fn mcp_running_command_recounts_new_output_and_can_resume_from_a_token_offset() {
    let mut session = McpSession::new();
    let [tab, _other] = create_tabs(&mut session);
    #[cfg(windows)]
    let command = "[Console]::WriteLine('FIRST_CHUNK')\n$null = [Console]::In.ReadLine()\n[Console]::Write('SECOND_CHUNK')";
    #[cfg(not(windows))]
    let command = "printf 'FIRST_CHUNK\\n'\nread -r line\nprintf 'SECOND_CHUNK'";
    let accepted = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab, "command": command, "separate_output": false}],
            "wait_timeout": 0.5_f64
        }),
    );
    let initial = command_result(&accepted);
    assert_eq!(initial.get("finished"), Some(&json!(false)));
    let first = initial.get("output").and_then(Value::as_str).unwrap();
    assert!(first.contains("FIRST_CHUNK"), "{accepted}");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let observed_tokens = tokenizer.count(first.as_bytes());
    let start_token = observed_tokens + 1;
    assert_eq!(initial.get("tokens"), Some(&json!(observed_tokens)));
    let id = initial.get("command_id").and_then(Value::as_str).unwrap();
    let waiting = session.call(
        "view",
        json!({"ids": [id], "wait_timeout": 0_u64, "start_token": 2_u64}),
    );
    assert_eq!(
        command_result(&waiting)
            .get("output")
            .and_then(Value::as_str),
        Some(expected_suffix(&tokenizer, first, 2))
    );
    assert_eq!(
        command_result(&waiting).get("tokens"),
        Some(&json!(observed_tokens))
    );
    let written = session.call(
        "manual_write",
        json!({"exec": [{"tab_id": tab, "text": "\r\n"}], "wait_timeout": 0_u64}),
    );
    assert!(entries(&written).first().unwrap().get("error").is_none());
    let completed = session.call("view", json!({"ids": [id], "wait_timeout": 5_u64}));
    let full = command_result(&completed);
    let output = full.get("output").and_then(Value::as_str).unwrap();
    let total = tokenizer.count(output.as_bytes());
    assert!(output.contains("SECOND_CHUNK"), "{completed}");
    assert!(total > observed_tokens);
    assert_eq!(full.get("tokens"), Some(&json!(total)));
    assert_eq!(full.get("finished"), Some(&json!(true)));
    let resumed = session.call(
        "view",
        json!({"ids": [id], "wait_timeout": 0_u64, "start_token": start_token}),
    );
    assert_eq!(
        command_result(&resumed)
            .get("output")
            .and_then(Value::as_str),
        Some(expected_suffix(&tokenizer, output, start_token))
    );
    assert_eq!(command_result(&resumed).get("tokens"), Some(&json!(total)));
    drop(session);
}
