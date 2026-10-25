use super::execution::SHELL;
use crate::support::{locked_with_env, parse_command_id, parse_tab_created, run_cli};
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
    assert!(text.contains("<OUTPUT>\nhello world\n</OUTPUT>"), "{text}");
    assert!(
        text.contains("<TOTAL_OUTPUT_TOKENS>\n2\n</TOTAL_OUTPUT_TOKENS>"),
        "{text}"
    );
    let id = parse_command_id(&accepted);
    for (offset, expected) in [("2", " world"), ("3", ""), ("9999999", "")] {
        let viewed = run_cli(&["view", &id, "--start-token", offset]);
        assert!(viewed.status.success(), "{viewed:?}");
        let document = String::from_utf8(viewed.stdout).unwrap();
        let output_tag = if expected.is_empty() {
            "<OUTPUT>\n</OUTPUT>".to_owned()
        } else {
            format!("<OUTPUT>\n{expected}\n</OUTPUT>")
        };
        assert!(document.contains(&output_tag), "{document}");
        assert!(
            document.contains("<TOTAL_OUTPUT_TOKENS>\n2\n</TOTAL_OUTPUT_TOKENS>"),
            "{document}"
        );
    }
    let full = run_cli(&["view", &id]);
    assert!(full.status.success(), "{full:?}");
    assert!(
        String::from_utf8(full.stdout.clone())
            .unwrap()
            .contains("<OUTPUT>\nhello world\n</OUTPUT>")
    );
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
