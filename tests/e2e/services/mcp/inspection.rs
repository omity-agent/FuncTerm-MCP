use super::{
    execution::create_tabs,
    wire::{McpSession, assert_field, command_result, entries, field, result, text},
};
use sugar_path::SugarPath as _;
#[test]
fn mcp_views_preserve_mixed_id_order_errors_and_shared_completion() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    let command = session.execute(&first, "echo VIEW_ID_LIST_RESULT", true, 5.0);
    let command_id = text(command_result(&command), "/command_id");
    let first_cwd = text(result(&command), "/shell/cwd");
    let second_cwd = crate::support::temp_root()
        .normalize()
        .to_slash()
        .into_owned();
    assert_ne!(first_cwd, second_cwd);
    let viewed = session.view(&[&second, command_id, "missing", &first, &second], 1, 0.0);
    let results = entries(&viewed);
    assert_eq!(results.len(), 5);
    assert_field(&viewed, "/result/isError", true);
    for (index, cwd) in [
        (0, second_cwd.as_str()),
        (3, first_cwd),
        (4, second_cwd.as_str()),
    ] {
        let entry = results
            .get(index)
            .unwrap_or_else(|| panic!("missing result {index}: {viewed}"));
        assert_eq!(text(entry, "/result/shell/cwd"), cwd);
        assert!(entry.pointer("/result/screen").is_some());
    }
    let completed = field(
        results
            .get(1)
            .unwrap_or_else(|| panic!("missing command: {viewed}")),
        "/result/command",
    );
    assert_field(completed, "/finished", true);
    assert!(text(completed, "/stdout").contains("VIEW_ID_LIST_RESULT"));
    assert!(
        text(
            results
                .get(2)
                .unwrap_or_else(|| panic!("missing error: {viewed}")),
            "/error"
        )
        .contains("unknown id missing")
    );
    super::boundaries::text_documents(&viewed);
    #[cfg(windows)]
    let source = "Start-Sleep -Seconds 2\nWrite-Output MANY_WAITERS_FINISHED";
    #[cfg(not(windows))]
    let source = "sleep 2\necho MANY_WAITERS_FINISHED";
    let accepted = session.execute(&first, source, true, 0.0);
    let pending_id = text(command_result(&accepted), "/command_id");
    let waited = session.view(&[pending_id; 64], 1, 5.0);
    assert_eq!(entries(&waited).len(), 64);
    for entry in entries(&waited) {
        let saved = field(entry, "/result/command");
        assert_field(saved, "/finished", true);
        assert_field(saved, "/exit_code", 0_i32);
        assert!(
            text(saved, "/stdout").contains("MANY_WAITERS_FINISHED"),
            "{waited}"
        );
    }
}
