use super::{
    execution::{SHELL, create_tabs},
    wire::{McpSession, entries},
};
use crate::support::{locked_with_env, parse_tab_created, run_cli, send_command};
use rmcp::serde_json::{Value, json};

#[cfg(windows)]
const OUTPUT_COMMAND: &str = "Write-Output 'RAW<tag> & text'";
#[cfg(not(windows))]
const OUTPUT_COMMAND: &str = "printf '%s\\n' 'RAW<tag> & text'";

#[test]
fn mcp_xml_documents_stay_on_separate_lines_across_batches_and_calls() {
    let mut session = McpSession::new();
    let created = session.call(
        "new_tab",
        json!({"exec": [{"starting_shell": SHELL}, {"starting_shell": SHELL}]}),
    );
    let creation_text = text_documents(&created).concat();
    assert!(creation_text.contains("</TAB_ID>\n<TAB_ID>\n"));
    let tab_id = entries(&created)
        .first()
        .unwrap()
        .pointer("/result/tab_id")
        .and_then(Value::as_str)
        .unwrap();
    let accepted = session.call(
        "send_command",
        json!({
            "exec": [{"tab_id": tab_id, "command": OUTPUT_COMMAND}],
            "wait_timeout": 5_u64,
        }),
    );
    let command_id = entries(&accepted)
        .first()
        .unwrap()
        .pointer("/result/command/command_id")
        .and_then(Value::as_str)
        .unwrap();
    let viewed = session.call(
        "view",
        json!({"ids": [command_id, tab_id], "wait_timeout": 0_u64}),
    );
    let command_text = text_documents(&accepted).concat();
    let view_text = text_documents(&viewed).concat();
    assert!(command_text.contains("RAW<tag> & text"));
    assert!(view_text.contains("</NOTE>\n<SHELL>\n"));
    let consecutive = [creation_text, command_text, view_text].concat();
    assert!(consecutive.contains("</TAB_ID>\n<SHELL>\n"));
    assert!(consecutive.contains("</NOTE>\n<SHELL>\n"));
    assert!(!consecutive.contains("</NOTE><SHELL>"));
    drop(session);
}

#[test]
fn mcp_error_blocks_are_separated_without_changing_structured_errors() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let response = session.call(
        "view",
        json!({
            "ids": ["missing-before", first, "missing-middle", second, "missing-after"],
            "wait_timeout": 0_u64,
        }),
    );
    let documents = text_documents(&response);
    assert_eq!(documents.len(), 5);
    for (entry, document) in entries(&response).iter().zip(&documents) {
        if let Some(error) = entry.get("error").and_then(Value::as_str) {
            assert_eq!(document.strip_suffix('\n'), Some(error));
        }
    }
    for pair in documents.windows(2) {
        let previous = pair.first().unwrap();
        let following = pair.last().unwrap();
        assert_eq!(
            [*previous, *following].concat().lines().count(),
            previous.lines().count() + following.lines().count(),
        );
    }
    drop(session);
}

#[test]
fn cli_xml_documents_end_with_one_newline() {
    let guard = locked_with_env(&[]);
    let created = run_cli(&["new-tab", "--no-profile", "--starting-shell", SHELL]);
    let tab = parse_tab_created(&created);
    let accepted = send_command(&tab.tab_id, OUTPUT_COMMAND, 5.0);
    let viewed = run_cli(&["view", &tab.tab_id]);
    let closed = run_cli(&["close", "--tab-id", &tab.tab_id]);
    for response in [created, accepted, viewed, closed] {
        assert!(response.status.success(), "{response:?}");
        let text = String::from_utf8(response.stdout).unwrap();
        assert!(text.ends_with(">\n"), "{text:?}");
        assert!(!text.ends_with("\n\n"), "{text:?}");
        assert!(!text.starts_with('\n'), "{text:?}");
    }
    drop(guard);
}

pub(super) fn text_documents(response: &Value) -> Vec<&str> {
    let blocks = response
        .pointer("/result/content")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(blocks.len(), entries(response).len());
    blocks
        .iter()
        .map(|block| {
            let text = block.get("text").and_then(Value::as_str).unwrap();
            assert!(text.ends_with('\n'), "unterminated text document: {text:?}");
            assert!(!text.ends_with("\n\n"), "extra blank line: {text:?}");
            assert!(!text.starts_with('\n'), "leading blank line: {text:?}");
            text
        })
        .collect()
}
