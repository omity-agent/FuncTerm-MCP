use super::{
    boundaries::text_documents,
    execution::create_tabs,
    wire::{McpSession, entries},
};
use rmcp::serde_json::{Value, json};
#[cfg(windows)]
const OUTPUT_COMMAND: &str = "[Console]::Write(\"hello world 你好 🧠🚀 <|endoftext|>`nTAIL\")";
#[cfg(not(windows))]
const OUTPUT_COMMAND: &str = "printf '%s\\n%s' 'hello world 你好 🧠🚀 <|endoftext|>' 'TAIL'";
#[test]
fn mcp_token_offsets_are_absolute_unicode_safe_and_preserve_total_count() {
    let mut session = McpSession::new();
    let [tab, _other] = create_tabs(&mut session);
    let accepted = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab, "command": OUTPUT_COMMAND, "separate_output": false}],
            "wait_timeout": 5_u64
        }),
    );
    let command = command_result(&accepted);
    let output = command.get("output").and_then(Value::as_str).unwrap();
    let id = command.get("command_id").and_then(Value::as_str).unwrap();
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let total = tokenizer.count(output.as_bytes());
    assert!(output.contains("你好 🧠🚀"), "{accepted}");
    assert_eq!(command.get("tokens"), Some(&json!(total)));
    assert!(
        text_documents(&accepted)
            .concat()
            .contains(&format!("<TOKENS>\n{total}\n</TOKENS>"))
    );
    for start in (1..=total + 2).chain(core::iter::once(usize::MAX)) {
        let viewed = session.call(
            "view",
            json!({"ids": [id], "wait_timeout": 0_u64, "start_token": start}),
        );
        let result = command_result(&viewed);
        assert_eq!(
            result.get("output").and_then(Value::as_str),
            Some(expected_suffix(&tokenizer, output, start)),
            "start_token={start}: {viewed}"
        );
        assert_eq!(result.get("tokens"), Some(&json!(total)));
        assert_eq!(result.get("finished"), Some(&json!(true)));
        assert_eq!(result.get("exit_code"), Some(&json!(0_i32)));
    }
    let restored = session.call("view", json!({"ids": [id], "wait_timeout": 0_u64}));
    assert_eq!(
        command_result(&restored).get("output"),
        command.get("output")
    );
    drop(session);
}
#[test]
fn mcp_tab_screen_uses_the_same_token_offsets_without_mutating_the_screen() {
    let mut session = McpSession::new();
    let [tab, _other] = create_tabs(&mut session);
    let accepted = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab, "command": OUTPUT_COMMAND, "separate_output": false}],
            "wait_timeout": 5_u64
        }),
    );
    assert_eq!(
        command_result(&accepted).get("finished"),
        Some(&json!(true))
    );
    let full = session.call("view", json!({"ids": [tab], "wait_timeout": 0.1_f64}));
    let screen = entries(&full)
        .first()
        .unwrap()
        .pointer("/result/screen")
        .and_then(Value::as_str)
        .unwrap();
    assert!(screen.contains("TAIL"), "{full}");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let total = tokenizer.count(screen.as_bytes());
    for start in [1, 2, 3, total + 1, total + 2, usize::MAX] {
        let viewed = session.call(
            "view",
            json!({"ids": [tab], "wait_timeout": 0_u64, "start_token": start}),
        );
        assert_eq!(
            entries(&viewed)
                .first()
                .unwrap()
                .pointer("/result/screen")
                .and_then(Value::as_str),
            Some(expected_suffix(&tokenizer, screen, start)),
            "{viewed}"
        );
    }
    drop(session);
}
#[test]
fn mcp_empty_command_output_has_zero_tokens_at_every_offset() {
    let mut session = McpSession::new();
    let [tab, _other] = create_tabs(&mut session);
    #[cfg(windows)]
    let noop = "$null = 1";
    #[cfg(not(windows))]
    let noop = ":";
    let accepted = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab, "command": noop, "separate_output": false}],
            "wait_timeout": 5_u64
        }),
    );
    let command = command_result(&accepted);
    assert_eq!(command.get("tokens"), Some(&json!(0_u64)));
    assert_eq!(command.get("output"), Some(&json!("")));
    let id = command.get("command_id").and_then(Value::as_str).unwrap();
    for start in [1, 2, usize::MAX] {
        let viewed = session.call(
            "view",
            json!({"ids": [id], "wait_timeout": 0_u64, "start_token": start}),
        );
        assert_eq!(command_result(&viewed).get("tokens"), Some(&json!(0_u64)));
        assert_eq!(command_result(&viewed).get("output"), Some(&json!("")));
    }
    drop(session);
}
pub(super) fn command_result(response: &Value) -> &Value {
    entries(response)
        .first()
        .unwrap()
        .pointer("/result/command")
        .unwrap_or_else(|| panic!("missing command: {response}"))
}
pub(super) fn expected_suffix<'text>(
    tokenizer: &toktok::Tokenizer,
    text: &'text str,
    start_token: usize,
) -> &'text str {
    let ids = tokenizer.encode(text.as_bytes());
    let prefix = ids
        .get(..(start_token - 1).min(ids.len()))
        .unwrap_or_else(|| panic!("token prefix is outside its encoding"));
    let skipped = tokenizer.decode(prefix);
    text.get(text.floor_char_boundary(skipped.len())..).unwrap()
}
