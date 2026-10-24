use super::{
    execution::create_tabs,
    wire::{McpSession, command_result, entries, text},
};
use rmcp::serde_json::Value;
#[cfg(windows)]
const OUTPUT_COMMAND: &str = "Write-Output 'RAW<tag> & text'";
#[cfg(not(windows))]
const OUTPUT_COMMAND: &str = "printf '%s\\n' 'RAW<tag> & text'";
#[test]
fn mcp_xml_documents_stay_on_separate_lines_across_batches_and_calls() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let accepted = session.execute(&first, OUTPUT_COMMAND, false, 5.0);
    let id = text(command_result(&accepted), "/command_id");
    let viewed = session.view(&[id, &first, "missing", &second], 1, 0.0);
    let command_text = text_documents(&accepted).concat();
    let view_text = text_documents(&viewed).concat();
    assert!(command_text.contains("RAW<tag> & text"));
    assert!(view_text.contains("</NOTE>\n<SHELL>"));
    assert!(!command_text.ends_with('\n'));
    assert!(!view_text.ends_with('\n'));
}
pub(super) fn text_documents(response: &Value) -> Vec<&str> {
    let blocks = response
        .pointer("/result/content")
        .and_then(Value::as_array)
        .unwrap();
    assert_eq!(blocks.len(), entries(response).len());
    blocks
        .iter()
        .zip(entries(response))
        .enumerate()
        .map(|(index, (block, entry))| {
            let document = text(block, "/text");
            assert_eq!(
                document.ends_with('\n'),
                index + 1 < blocks.len(),
                "{document:?}"
            );
            assert!(!document.starts_with('\n'), "{document:?}");
            if let Some(error) = entry.get("error").and_then(Value::as_str) {
                assert_eq!(document.trim_end_matches('\n'), error);
            }
            document
        })
        .collect()
}
