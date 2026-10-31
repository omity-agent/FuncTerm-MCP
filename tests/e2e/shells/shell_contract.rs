use super::matrix::{assert_cwd, assert_shell_query, case_command, case_dir, shell_cases};
use crate::support::{create_tab, parse_command_result, parse_tab_view, run_cli, send_command};
use std::fs;
#[test]
fn cli_runs_commands_for_every_supported_shell() {
    let _guard = super::matrix::daemon();
    for case in shell_cases() {
        let start = case_dir(case.name, "start dir");
        let next = case_dir(case.name, "next dir");
        let created = create_tab(&start, case.name);
        let shell_before = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_before, &start, case.name);
        let command = case_command(case.name, &next);
        let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
        assert!(result.finished, "{}: {result:?}", case.name);
        assert!(result.stdout.contains("MCP_PTY_STDOUT"), "{result:?}");
        assert!(
            if case.name == "nu" {
                result.stdout.contains("MCP_PTY_STDERR")
            } else {
                result.stderr.contains("MCP_PTY_STDERR")
            },
            "{result:?}"
        );
        assert_eq!(
            result.exit_code,
            Some(case.expected_exit_code),
            "{result:?}"
        );
        assert_ne!(result.time_consumption, "0ns");
        assert_cwd(&result.cwd, &next, case.name);
        let shell_after = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_after, &next, case.name);
        super::state::assert_persistent_state(&created.tab_id, case.name);
        if case.name == "nu" {
            let marker = "MCP_PTY_NUSHELL_IMPLICIT.txt";
            fs::write(next.join(marker), "implicit output marker").unwrap();
            let implicit = parse_command_result(&send_command(
                &created.tab_id,
                &format!("ls | where name == {marker:?}"),
                10.0,
            ));
            assert_eq!(implicit.exit_code, Some(0_i32), "{implicit:?}");
            assert!(implicit.stdout.contains(marker), "{implicit:?}");
        }
    }
}
#[cfg(windows)]
#[test]
fn cli_exit_closes_shell_and_preserves_command_result() {
    let _guard = super::matrix::daemon();
    for shell in ["powershell", "nu"] {
        let created = create_tab(&case_dir(shell, "shell exit"), shell);
        let failure = if shell == "powershell" {
            "throw 'EXPECTED_SHELL_ERROR'"
        } else {
            "error make {msg: 'EXPECTED_SHELL_ERROR'}"
        };
        let failed = crate::support::complete_command(&created.tab_id, failure);
        assert!(failed.finished);
        assert_ne!(failed.exit_code, Some(0_i32));
        assert!(parse_tab_view(&run_cli(&["view", &created.tab_id])).alive);
        let output = send_command(&created.tab_id, "exit 42", 5.0);
        let command_id = crate::support::parse_command_id(&output);
        let exited = parse_command_result(&output);
        assert!(exited.finished, "{output:?}");
        assert_eq!(exited.exit_code, Some(42_i32), "{shell}: {output:?}");
        let closing = std::time::Instant::now();
        while parse_tab_view(&run_cli(&["view", &created.tab_id])).alive {
            assert!(
                closing.elapsed() < core::time::Duration::from_secs(3),
                "{shell} stayed alive after exit"
            );
            std::thread::sleep(core::time::Duration::from_millis(10));
        }
        let cached = parse_command_result(&run_cli(&["view", &command_id]));
        assert_eq!(cached.exit_code, Some(42_i32));
    }
}
