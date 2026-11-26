use super::{
    execution::create_tabs,
    wire::{
        McpSession, assert_field, assert_finished_output, command_result, entries, field, result,
        text,
    },
};
use itertools::Itertools as _;
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
    let viewed = session.view(
        &[
            (&second, 1),
            (command_id, 1),
            ("missing", 1),
            (&first, 1),
            (&second, 1),
        ],
        0.0,
    );
    let (second_view, command_view, missing, first_view, repeated) = entries(&viewed)
        .iter()
        .collect_tuple()
        .unwrap_or_else(|| panic!("unexpected view batch: {viewed}"));
    assert_field(&viewed, "/result/isError", true);
    for (entry, cwd) in [
        (second_view, second_cwd.as_str()),
        (first_view, first_cwd),
        (repeated, second_cwd.as_str()),
    ] {
        assert_eq!(text(entry, "/result/shell/cwd"), cwd);
        assert!(entry.pointer("/result/screen").is_some());
    }
    assert_finished_output(
        field(command_view, "/result/command"),
        "/stdout",
        "VIEW_ID_LIST_RESULT",
    );
    assert!(text(missing, "/error").contains("unknown id missing"));
    super::wire::text_documents(&viewed);
    #[cfg(windows)]
    let source = "Start-Sleep -Seconds 2\nWrite-Output MANY_WAITERS_FINISHED";
    #[cfg(not(windows))]
    let source = "sleep 2\necho MANY_WAITERS_FINISHED";
    let accepted = session.execute(&first, source, true, 0.0);
    let pending_id = text(command_result(&accepted), "/command_id");
    let waited = session.view(&[(pending_id, 1); 64], 5.0);
    assert_eq!(entries(&waited).len(), 64);
    for entry in entries(&waited) {
        assert_finished_output(
            field(entry, "/result/command"),
            "/stdout",
            "MANY_WAITERS_FINISHED",
        );
    }
    super::wire::text_documents(&waited);
}
