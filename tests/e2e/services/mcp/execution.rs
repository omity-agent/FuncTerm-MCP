extern crate alloc;
use super::wire::{
    McpSession, assert_field, assert_finished_output, command_result, completed_commands, entries,
    field, text,
};
pub(super) use crate::support::SHELL;
use alloc::rc::Rc;
use core::time::Duration;
use itertools::Itertools as _;
use rmcp::serde_json::json;
use std::time::Instant;
pub(super) fn create_tabs<const N: usize>(session: &mut McpSession) -> [String; N] {
    let tabs = (0..N)
        .map(|index| {
            json!({"starting_shell": SHELL, "starting_directory": (index != 0).then(crate::support::temp_root)})
        })
        .collect::<Vec<_>>();
    let response = session.call("new_tab", json!({"tabs": tabs}));
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
        "run",
        json!({
            "commands": [
                {"tab_id": first, "command": "echo FIRST_BATCH_RESULT", "separate_output": true},
                {"tab_id": "missing", "command": "echo UNUSED", "separate_output": true},
                {"tab_id": second, "command": "echo LAST_BATCH_RESULT", "separate_output": true}
            ], "wait_timeout": 5_u64
        }),
    );
    let (first_run, rejected, last_run) = entries(&response)
        .iter()
        .collect_tuple()
        .unwrap_or_else(|| panic!("unexpected batch: {response}"));
    assert_field(&response, "/result/isError", true);
    assert!(
        super::wire::text_documents(&response)
            .iter()
            .all(|block| !block.starts_with("commands["))
    );
    assert!(rejected.get("error").is_some());
    let ids = [first_run, last_run].map(|entry| text(entry, "/result/command/command_id"));
    let complete = completed_commands(&ids, |arguments| session.call("view", arguments));
    for (entry, expected) in entries(&complete)
        .iter()
        .zip(["FIRST_BATCH_RESULT", "LAST_BATCH_RESULT"])
    {
        assert_finished_output(field(entry, "/result/command"), "/stdout", expected);
    }
    let viewed = session.view(&[(&first, 1), (&second, 1)], 0.0);
    assert!(
        entries(&viewed)
            .iter()
            .all(|entry| entry.pointer("/result/screen").is_some())
    );
    let stopped = crate::support::run_cli(&["daemon", "stop"]);
    assert!(stopped.status.success(), "{stopped:?}");
    let written = session.call("manual_write", json!({
        "inputs": [{"tab_id": first, "text": "x", "bytes": [120_u8]}, {"tab_id": second}], "wait_timeout": 0_u64
    }));
    let errors = entries(&written);
    assert!(text(errors.first().unwrap(), "/error").contains("cannot be provided together"));
    assert!(text(errors.last().unwrap(), "/error").contains("either text or bytes"));
    let still_stopped = crate::support::run_cli(&["daemon", "stop"]);
    assert!(still_stopped.status.success(), "{still_stopped:?}");
    assert_eq!(
        String::from_utf8(still_stopped.stdout).unwrap().trim(),
        "daemon is not running"
    );
}
#[test]
fn mcp_scaled_wait_budget_is_shared_and_timeout_does_not_cancel_commands() {
    for (multiplier, timeout) in [("1", 1.5_f64), ("1.5", 1.0_f64), ("0.5", 3.0_f64)] {
        let daemon = Rc::new(crate::support::locked_with_env(&[]));
        let mut session = McpSession::on(
            &daemon,
            &[
                "--no-profile",
                "--wait-timeout-multiplier",
                multiplier,
                "--wait-timeout-min=1",
                "--wait-timeout-max=10",
            ],
        );
        let [first, second] = create_tabs(&mut session);
        let ready = session.execute_many(&[&first, &second], "echo BATCH_READY", true, 10.0);
        for entry in entries(&ready) {
            assert_finished_output(field(entry, "/result/command"), "/stdout", "BATCH_READY");
        }
        let window = Duration::from_millis(1400)..Duration::from_millis(2800);
        let viewing = Instant::now();
        session.view(&[(&first, 1), (&second, 1)], timeout);
        assert!(window.contains(&viewing.elapsed()));
        #[cfg(windows)]
        let source = "Start-Sleep -Seconds 4\nWrite-Output BATCH_FINISHED";
        #[cfg(not(windows))]
        let source = "sleep 4\necho BATCH_FINISHED";
        let started = Instant::now();
        let response = session.execute_many(&[&first, &second], source, true, timeout);
        let elapsed = started.elapsed();
        assert!(window.contains(&elapsed), "{elapsed:?}: {response}");
        let queries = entries(&response)
            .iter()
            .map(|entry| {
                assert_field(entry, "/result/command/finished", false);
                (text(entry, "/result/command/command_id"), 1)
            })
            .collect::<Vec<_>>();
        let writing = Instant::now();
        let written = session.call("manual_write", json!({
            "inputs": [{"tab_id": first, "text": ""}, {"tab_id": second, "text": ""}], "wait_timeout": timeout
        }));
        assert_field(&written, "/result/isError", false);
        assert!(window.contains(&writing.elapsed()), "{written}");
        let completed = session.view(&queries, 8.0);
        for entry in entries(&completed) {
            assert_finished_output(field(entry, "/result/command"), "/stdout", "BATCH_FINISHED");
        }
    }
}
#[test]
fn script_run_uses_the_largest_explicit_timeout_and_defaults_only_when_all_omit() {
    let daemon = Rc::new(crate::support::locked_with_env(&[(
        "FUNCTERM_MCP_WAIT_TIMEOUT_DEFAULT",
        "6",
    )]));
    let mut session = McpSession::on(
        &daemon,
        &[
            "--single-parameter-run",
            "--wait-timeout-min=0",
            "--no-profile",
            "--output-mode=separated",
        ],
    );
    let [first, second] = create_tabs(&mut session);
    for script in [
        format!(
            "*** tab_id: {first};\necho UNUSED\n*** tab_id: {second}; wait_timeout: NaN;\necho UNUSED"
        ),
        format!("*** tab_id: {first};\necho UNUSED\n*** tab_id: {first};\necho UNUSED"),
        format!("*** tab_id: {first};\n"),
    ] {
        let rejected = session.call("run", json!({"script": script}));
        assert_field(&rejected, "/result/isError", true);
        assert!(
            rejected.pointer("/result/structuredContent").is_none(),
            "{rejected}"
        );
    }
    #[cfg(windows)]
    let slow = "Start-Sleep -Seconds 4\nWrite-Output SLOW_RESULT";
    #[cfg(not(windows))]
    let slow = "sleep 4\necho SLOW_RESULT";
    let mixed = format!(
        "*** tab_id: {first}; wait_timeout: 0;\necho FAST_RESULT\n*** tab_id: {second}; wait_timeout: 1.5;\n{slow}\n*** tab_id: missing;\necho UNUSED"
    );
    let started = Instant::now();
    let mixed_response = session.call("run", json!({"script": mixed}));
    assert!(
        started.elapsed() >= Duration::from_millis(1400),
        "{mixed_response}"
    );
    let (fast, pending, rejected) = entries(&mixed_response)
        .iter()
        .collect_tuple()
        .unwrap_or_else(|| panic!("unexpected script response: {mixed_response}"));
    assert_field(pending, "/result/command/finished", false);
    assert!(rejected.get("error").is_some());
    let fast_id = text(fast, "/result/command/command_id");
    let fast_response = completed_commands(&[fast_id], |arguments| session.call("view", arguments));
    assert_finished_output(command_result(&fast_response), "/stdout", "FAST_RESULT");
    let script = format!("*** tab_id: {first};\n{slow}");
    let response = session.call("run", json!({"script": script}));
    assert_finished_output(command_result(&response), "/stdout", "SLOW_RESULT");
}
