use super::{
    execution::create_tabs,
    wire::{McpSession, assert_field, assert_saved, command_result, result, text, text_documents},
};
use crate::support::tagged_content;
#[cfg(windows)]
const OUTPUT_COMMAND: &str = "[Console]::Write(\"hello world 你好 🧠🚀 <|endoftext|>`nTAIL\")";
#[cfg(not(windows))]
const OUTPUT_COMMAND: &str = "printf '%s\\n%s' 'hello world 你好 🧠🚀 <|endoftext|>' 'TAIL'";
#[test]
fn token_offsets_preserve_unicode_totals_and_saved_command_and_screen_output() {
    let mut session = McpSession::new();
    let [tab] = create_tabs(&mut session);
    let accepted = session.execute(&tab, OUTPUT_COMMAND, false, 5.0);
    let command = command_result(&accepted);
    let output = text(command, "/output");
    let id = text(command, "/command_id");
    let tokenizer = toktok::Tokenizer::builtin("o200k_base").unwrap();
    let total = tokenizer.count(output.as_bytes());
    assert!(output.contains("你好 🧠🚀"), "{accepted}");
    assert_field(command, "/total_output_tokens", total);
    assert_eq!(
        tagged_content(&text_documents(&accepted).concat(), "TOTAL_OUTPUT_TOKENS"),
        Some(format!("\n{total}\n").as_str())
    );
    for start in (1..=total + 2).chain(core::iter::once(usize::MAX)) {
        let viewed = session.view(&[id], start, 0.0);
        let saved = command_result(&viewed);
        assert_eq!(
            text(saved, "/output"),
            expected_suffix(&tokenizer, output, start),
            "{viewed}"
        );
        assert_field(saved, "/total_output_tokens", total);
        assert_field(saved, "/finished", true);
        assert_field(saved, "/exit_code", 0_i32);
    }
    let restored = session.view(&[id], 1, 0.0);
    assert_saved(&accepted, &restored);
    let full = session.view(&[&tab], 1, 0.1);
    let screen = text(result(&full), "/screen");
    assert!(screen.contains("TAIL"), "{full}");
    let screen_tokens = tokenizer.count(screen.as_bytes());
    for start in [1, 2, 3, screen_tokens + 1, screen_tokens + 2, usize::MAX] {
        let viewed = session.view(&[&tab], start, 0.0);
        assert_eq!(
            text(result(&viewed), "/screen"),
            expected_suffix(&tokenizer, screen, start),
            "{viewed}"
        );
    }
    #[cfg(windows)]
    let noop = "$null = 1";
    #[cfg(not(windows))]
    let noop = ":";
    let empty = session.execute(&tab, noop, false, 5.0);
    let empty_command = command_result(&empty);
    assert_field(empty_command, "/total_output_tokens", 0_u64);
    assert_field(empty_command, "/output", "");
    let empty_id = text(empty_command, "/command_id");
    for start in [1, 2, usize::MAX] {
        let viewed = session.view(&[empty_id], start, 0.0);
        assert_field(command_result(&viewed), "/total_output_tokens", 0_u64);
        assert_field(command_result(&viewed), "/output", "");
    }
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
