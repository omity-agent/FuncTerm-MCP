use super::{
    execution::create_tabs,
    pagination::expected_suffix,
    wire::{McpSession, assert_field, assert_saved, command_result, entries, text},
};
use rmcp::serde_json::json;
#[test]
fn mcp_separated_output_counts_both_streams_and_rejects_skipping_tokens() {
    let mut session = McpSession::new();
    let [tab] = create_tabs(&mut session);
    #[cfg(windows)]
    let command =
        "Write-Output 'hello world'\nWrite-Error -Message '你好 🧠' -ErrorAction Continue";
    #[cfg(not(windows))]
    let command = "printf 'hello world'\nprintf '你好 🧠' >&2";
    let accepted = session.execute(&tab, command, true, 5.0);
    let result = command_result(&accepted);
    let stdout = text(result, "/stdout");
    let stderr = text(result, "/stderr");
    assert_eq!(stdout.trim_end(), "hello world");
    assert!(stderr.contains("你好 🧠"), "{accepted}");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    assert_field(
        result,
        "/total_output_tokens",
        tokenizer.count(stdout.as_bytes()) + tokenizer.count(stderr.as_bytes()),
    );
    let id = text(result, "/command_id");
    let rejected = session.view(&[id, &tab], 2, 0.0);
    let results = entries(&rejected);
    assert!(text(results.first().unwrap(), "/error").contains("start_token must be 1"));
    assert!(results.last().unwrap().pointer("/result/screen").is_some());
    let unchanged = session.view(&[id], 1, 0.0);
    assert_saved(&accepted, &unchanged);
}
#[test]
fn mcp_running_command_recounts_new_output_and_can_resume_from_a_token_offset() {
    let mut session = McpSession::new();
    let [tab] = create_tabs(&mut session);
    #[cfg(windows)]
    let command = "[Console]::WriteLine('FIRST_CHUNK')\n$null = [Console]::In.ReadLine()\n[Console]::Write('SECOND_CHUNK')";
    #[cfg(not(windows))]
    let command = "printf 'FIRST_CHUNK\\n'\nread -r line\nprintf 'SECOND_CHUNK'";
    let accepted = session.execute(&tab, command, false, 0.5);
    let initial = command_result(&accepted);
    assert_field(initial, "/finished", false);
    let first = text(initial, "/output");
    assert!(first.contains("FIRST_CHUNK"), "{accepted}");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let observed_tokens = tokenizer.count(first.as_bytes());
    let start_token = observed_tokens + 1;
    assert_field(initial, "/total_output_tokens", observed_tokens);
    let id = text(initial, "/command_id");
    let waiting = session.view(&[id], 2, 0.0);
    assert_eq!(
        text(command_result(&waiting), "/output"),
        expected_suffix(&tokenizer, first, 2)
    );
    assert_field(
        command_result(&waiting),
        "/total_output_tokens",
        observed_tokens,
    );
    let written = session.call(
        "manual_write",
        json!({"exec": [{"tab_id": tab, "text": "\r\n"}], "wait_timeout": 0_u64}),
    );
    assert!(entries(&written).first().unwrap().get("error").is_none());
    let completed = session.view(&[id], 1, 5.0);
    let full = command_result(&completed);
    let output = text(full, "/output");
    let total = tokenizer.count(output.as_bytes());
    assert!(output.contains("SECOND_CHUNK"), "{completed}");
    assert!(total > observed_tokens);
    assert_field(full, "/total_output_tokens", total);
    assert_field(full, "/finished", true);
    let resumed = session.view(&[id], start_token, 0.0);
    assert_eq!(
        text(command_result(&resumed), "/output"),
        expected_suffix(&tokenizer, output, start_token)
    );
    assert_field(command_result(&resumed), "/total_output_tokens", total);
}
