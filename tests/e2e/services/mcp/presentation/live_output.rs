use super::{
    execution::create_tabs,
    pagination::expected_suffix,
    wire::{McpSession, assert_field, command_result, entries, text},
};
use rmcp::serde_json::json;
#[test]
fn mcp_separated_output_counts_both_streams_and_rejects_skipping_tokens() {
    let mut session = McpSession::new();
    let created = session.call("new_tab", json!({"tabs": [{"starting_shell": "python"}]}));
    let tab = text(super::wire::result(&created), "/tab_id").to_owned();
    let command = r"import sys
_ = input()
_ = sys.stdout.buffer.write(b'\x1b[31mhello world \x1b[3')
sys.stdout.flush()
_ = sys.stderr.buffer.write(b'\x1b]8;;https://example.invalid')
sys.stderr.flush()
_ = input()
_ = sys.stdout.buffer.write(b'1mnext \xe4')
sys.stdout.flush()
_ = sys.stderr.buffer.write(b'\x1b\\\xe4')
sys.stderr.flush()
_ = input()
_ = sys.stdout.buffer.write(b'\xbd\xa0\xe5\xa5\xbd \xf0\x9f\xa7\xa0')
_ = sys.stderr.buffer.write(b'\xbd\xa0\xe5\xa5\xbd \xf0\x9f\xa7\xa0')
";
    let started = session.execute(&tab, command, true, 0.5);
    let id = text(command_result(&started), "/command_id").to_owned();
    let input = json!({"inputs": [{"tab_id": tab, "text": "\r"}], "wait_timeout": 0_u64});
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    for expected in ["hello world ", "hello world next "] {
        session.call("manual_write", input.clone());
        let deadline = std::time::Instant::now() + core::time::Duration::from_secs(15);
        loop {
            let partial = session.view(&[(&id, 1)], 0.1);
            let snapshot = command_result(&partial);
            assert_field(snapshot, "/finished", false);
            if text(snapshot, "/stdout") == expected {
                assert_field(snapshot, "/stderr", "");
                assert_field(
                    snapshot,
                    "/total_output_tokens",
                    tokenizer.count(expected.as_bytes()),
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline, "{partial}");
        }
    }
    session.call("manual_write", input);
    let accepted = session.view(&[(&id, 1)], 5.0);
    let result = command_result(&accepted);
    assert_field(result, "/finished", true);
    assert_field(result, "/exit_code", 0_i32);
    let stdout = text(result, "/stdout");
    let stderr = text(result, "/stderr");
    assert_eq!(stdout, "hello world next 你好 🧠");
    assert!(stderr.contains("你好 🧠"), "{accepted}");
    assert_field(
        result,
        "/total_output_tokens",
        tokenizer.count(stdout.as_bytes()) + tokenizer.count(stderr.as_bytes()),
    );
    let rejected = session.view(&[(&id, 2), (&tab, 1), (&id, 1)], 0.0);
    let results = entries(&rejected);
    assert!(text(results.first().unwrap(), "/error").contains("start_token must be 1"));
    let screen = results
        .get(1)
        .unwrap_or_else(|| panic!("missing screen query: {rejected}"));
    assert!(screen.pointer("/result/screen").is_some());
    assert_eq!(
        result,
        super::wire::field(results.last().unwrap(), "/result/command")
    );
}
#[test]
fn mcp_running_command_recounts_new_output_and_can_resume_from_a_token_offset() {
    let mut session = McpSession::new();
    let [tab] = create_tabs(&mut session);
    #[cfg(windows)]
    let command = "[Console]::Write(\"`e[?1049h`e[2J`e[HFIRST_CHUNK`n\")\n$null = [Console]::In.ReadLine()\n[Console]::Write(\"`e[?1049lFIRST_CHUNK`nSECOND_CHUNK\")";
    #[cfg(not(windows))]
    let command = "printf '\\033[?1049h\\033[2J\\033[HFIRST_CHUNK\\n'\nread -r line\nprintf '\\033[?1049lFIRST_CHUNK\\nSECOND_CHUNK'";
    let accepted = session.execute(&tab, command, false, 0.5);
    let id = text(command_result(&accepted), "/command_id").to_owned();
    let mut visible = accepted;
    let deadline = std::time::Instant::now() + core::time::Duration::from_secs(15);
    while !text(command_result(&visible), "/output").contains("FIRST_CHUNK") {
        assert!(std::time::Instant::now() < deadline, "{visible}");
        visible = session.view(&[(&id, 1)], 1.0);
    }
    let initial = command_result(&visible);
    assert_field(initial, "/finished", false);
    let first = text(initial, "/output");
    assert!(first.contains("FIRST_CHUNK"), "{visible}");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let observed_tokens = tokenizer.count(first.as_bytes());
    let start_token = observed_tokens + 1;
    assert_field(initial, "/total_output_tokens", observed_tokens);
    let waiting = session.view(&[(&id, 2)], 0.0);
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
        json!({"inputs": [{"tab_id": tab, "text": "\r\n"}], "wait_timeout": 0_u64}),
    );
    assert!(entries(&written).first().unwrap().get("error").is_none());
    let completed = session.view(&[(&id, 1)], 5.0);
    let full = command_result(&completed);
    let output = text(full, "/output");
    let total = tokenizer.count(output.as_bytes());
    assert!(output.contains("SECOND_CHUNK"), "{completed}");
    assert!(total > observed_tokens);
    assert_field(full, "/total_output_tokens", total);
    assert_field(full, "/finished", true);
    let resumed = session.view(&[(&id, start_token)], 0.0);
    assert_eq!(
        text(command_result(&resumed), "/output"),
        expected_suffix(&tokenizer, output, start_token)
    );
    assert_field(command_result(&resumed), "/total_output_tokens", total);
}
