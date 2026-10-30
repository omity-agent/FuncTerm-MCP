use super::execution::SHELL;
use crate::support::{
    assert_tag_randomization, locked_with_env, parse_command_id, parse_tab_created, run_cli,
    tagged_content,
};
#[test]
fn cli_reports_total_tokens_and_forwards_the_view_start_token() {
    let guard = locked_with_env(&[]);
    let created = run_cli(&["new-tab", "--no-profile", "--starting-shell", SHELL]);
    let tab = parse_tab_created(&created);
    #[cfg(windows)]
    let command = "[Console]::Write('hello world')";
    #[cfg(not(windows))]
    let command = "printf 'hello world'";
    let accepted = run_cli(&[
        "send-command",
        &tab.tab_id,
        "--command",
        command,
        "--separate-output",
        "false",
        "--wait-timeout",
        "5",
    ]);
    assert!(accepted.status.success(), "{accepted:?}");
    let text = String::from_utf8(accepted.stdout.clone()).unwrap();
    assert_eq!(tagged_content(&text, "OUTPUT"), Some("\nhello world\n"));
    assert_eq!(tagged_content(&text, "TOTAL_OUTPUT_TOKENS"), Some("\n2\n"));
    let id = parse_command_id(&accepted);
    for (offset, expected) in [("2", "\n world\n"), ("3", "\n"), ("9999999", "\n")] {
        let viewed = run_cli(&["view", &id, "--start-token", offset]);
        assert!(viewed.status.success(), "{viewed:?}");
        let document = String::from_utf8(viewed.stdout).unwrap();
        assert_eq!(tagged_content(&document, "OUTPUT"), Some(expected));
        assert_eq!(
            tagged_content(&document, "TOTAL_OUTPUT_TOKENS"),
            Some("\n2\n")
        );
    }
    let full = run_cli(&["view", &id]);
    assert!(full.status.success(), "{full:?}");
    let documents: Vec<_> = core::iter::repeat_with(|| {
        let viewed = run_cli(&["view", &id]);
        assert!(viewed.status.success(), "{viewed:?}");
        let document = String::from_utf8(viewed.stdout).unwrap();
        assert_eq!(tagged_content(&document, "OUTPUT"), Some("\nhello world\n"));
        document
    })
    .take(64)
    .collect();
    assert_tag_randomization(documents.iter().map(String::as_str));
    let closed = run_cli(&["close", "--tab-id", &tab.tab_id]);
    for response in [&created, &accepted, &full, &closed] {
        assert!(response.status.success(), "{response:?}");
        let document = core::str::from_utf8(&response.stdout).unwrap();
        assert!(document.ends_with(">\n"), "{document:?}");
        assert!(
            !document.ends_with("\n\n") && !document.starts_with('\n'),
            "{document:?}"
        );
    }
    for invalid_start in ["0", "-1"] {
        let invalid = run_cli(&["view", &id, "--start-token", invalid_start]);
        assert!(!invalid.status.success());
    }
    drop(guard);
}
