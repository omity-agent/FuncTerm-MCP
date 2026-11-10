extern crate alloc;
use super::wire::{McpSession, assert_field, entries, text};
use alloc::rc::Rc;
use core::time::Duration;
use rmcp::serde_json::json;
use std::time::Instant;
#[cfg(windows)]
pub(super) const SHELL: &str = "powershell";
#[cfg(not(windows))]
pub(super) const SHELL: &str = "bash";
pub(super) fn create_tabs<const N: usize>(session: &mut McpSession) -> [String; N] {
    let exec = (0..N)
        .map(|index| {
            json!({"starting_shell": SHELL, "starting_directory": (index != 0).then(crate::support::temp_root)})
        })
        .collect::<Vec<_>>();
    let response = session.call("new_tab", json!({"exec": exec}));
    let created = entries(&response);
    assert_eq!(created.len(), N);
    super::wire::text_documents(&response);
    core::array::from_fn(|index| {
        let entry = created
            .get(index)
            .unwrap_or_else(|| panic!("missing tab {index}"));
        text(entry, "/result/tab_id").to_owned()
    })
}
#[test]
fn mcp_batch_preserves_order_and_isolates_errors() {
    let mut session = McpSession::new();
    let [first, second] = create_tabs(&mut session);
    assert_ne!(first, second);
    let response = session.call(
        "send_command",
        json!({
            "exec": [
                {"tab_id": first, "command": "echo FIRST_BATCH_RESULT", "separate_output": true},
                {"tab_id": "missing", "command": "echo UNUSED", "separate_output": true},
                {"tab_id": second, "command": "echo LAST_BATCH_RESULT", "separate_output": true}
            ], "wait_timeout": 5_u64
        }),
    );
    let results = entries(&response);
    assert_field(&response, "/result/isError", true);
    assert_eq!(results.len(), 3);
    let documents = super::wire::text_documents(&response);
    assert!(documents.iter().all(|block| !block.starts_with("exec[")));
    assert!(
        text(results.first().unwrap(), "/result/command/stdout").contains("FIRST_BATCH_RESULT")
    );
    assert!(
        results
            .get(1)
            .is_some_and(|entry| entry.get("error").is_some())
    );
    assert!(text(results.last().unwrap(), "/result/command/stdout").contains("LAST_BATCH_RESULT"));
    let viewed = session.view(&[&first, &second], 1, 0.0);
    assert!(
        entries(&viewed)
            .iter()
            .all(|entry| entry.pointer("/result/screen").is_some())
    );
    let written = session.call("manual_write", json!({
        "exec": [{"tab_id": first, "text": "x", "bytes": [120_u8]}, {"tab_id": second}], "wait_timeout": 0_u64
    }));
    let errors = entries(&written);
    assert!(text(errors.first().unwrap(), "/error").contains("cannot be provided together"));
    assert!(text(errors.last().unwrap(), "/error").contains("either text or bytes"));
}
#[test]
fn mcp_scaled_wait_budget_is_shared_and_timeout_does_not_cancel_commands() {
    for (multiplier, timeout) in [("1", 1.5_f64), ("1.5", 1.0_f64), ("0.5", 3.0_f64)] {
        let daemon = Rc::new(crate::support::locked_with_env(&[]));
        let mut session = McpSession::on(
            &daemon,
            &["--no-profile", "--wait-timeout-multiplier", multiplier],
        );
        let [first, second] = create_tabs(&mut session);
        let initialization = "echo BATCH_READY";
        let ready = session.call(
            "send_command",
            json!({
                "exec": [
                    {"tab_id": first, "command": initialization, "separate_output": true},
                    {"tab_id": second, "command": initialization, "separate_output": true}
                ], "wait_timeout": 10_u64
            }),
        );
        for entry in entries(&ready) {
            assert_field(entry, "/result/command/finished", true);
            assert_field(entry, "/result/command/exit_code", 0_i32);
            assert!(text(entry, "/result/command/stdout").contains("BATCH_READY"));
        }
        let window = Duration::from_millis(1400)..Duration::from_millis(2800);
        let viewing = Instant::now();
        session.view(&[&first, &second], 1, timeout);
        assert!(window.contains(&viewing.elapsed()));
        #[cfg(windows)]
        let source = "Start-Sleep -Seconds 4\nWrite-Output BATCH_FINISHED";
        #[cfg(not(windows))]
        let source = "sleep 4\necho BATCH_FINISHED";
        let started = Instant::now();
        let response = session.call(
            "send_command",
            json!({
                "exec": [
                    {"tab_id": first, "command": source, "separate_output": true},
                    {"tab_id": second, "command": source, "separate_output": true}
                ], "wait_timeout": timeout
            }),
        );
        let elapsed = started.elapsed();
        assert!(window.contains(&elapsed), "{elapsed:?}: {response}");
        let ids = entries(&response)
            .iter()
            .map(|entry| {
                assert_field(entry, "/result/command/finished", false);
                text(entry, "/result/command/command_id")
            })
            .collect::<Vec<_>>();
        let writing = Instant::now();
        let written = session.call("manual_write", json!({
            "exec": [{"tab_id": first, "text": ""}, {"tab_id": second, "text": ""}], "wait_timeout": timeout
        }));
        assert_field(&written, "/result/isError", false);
        assert!(window.contains(&writing.elapsed()), "{written}");
        let completed = session.view(&ids, 1, 8.0);
        for entry in entries(&completed) {
            assert_field(entry, "/result/command/finished", true);
            assert!(text(entry, "/result/command/stdout").contains("BATCH_FINISHED"));
        }
    }
}
