#[cfg(windows)]
#[path = "cli/history.rs"]
mod history;
#[cfg(windows)]
#[path = "cli/shell_matrix.rs"]
mod shell_matrix;
#[cfg(windows)]
#[path = "cli/support.rs"]
mod support;
#[cfg(windows)]
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use super::support::{
        create_shell, locked, parse_command_query, run_cli, run_cli_with_pipes, send_test_command,
        write_keyboard,
    };
    use core::time::Duration;
    use std::thread;
    use std::time::Instant;
    #[test]
    fn cli_rejects_missing_cwd() {
        let _guard = locked();
        let missing = std::env::temp_dir().join("definitely-missing-mcp-pty-cli-cwd");
        let output = run_cli(&[
            "new-shell",
            "--cwd",
            missing.to_str().unwrap(),
            "--shell",
            "powershell",
        ]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("cwd does not exist or is not a directory")
        );
    }
    #[test]
    fn cli_pipe_capture_returns_without_hanging() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let output = run_cli_with_pipes(&[
            "new-shell",
            "--cwd",
            cwd.to_str().unwrap(),
            "--shell",
            "powershell",
        ]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("shell_id: "));
    }
    #[test]
    fn cli_query_returns_command_output() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_shell(&cwd, "powershell");
        let accepted_output = send_test_command(&created.shell_id);
        let query = parse_command_query(&accepted_output);
        assert_eq!(
            query.recognized_as, "command",
            "query kind should be command"
        );
        assert!(!query.cwd.is_empty(), "cwd should be reported");
        assert!(query.finished, "command should be finished");
        assert!(
            query.stdout.contains("MCP_PTY_TEST"),
            "stdout should include test marker"
        );
        assert_eq!(query.stderr, "", "stderr should be empty");
        assert_eq!(query.exit_code, Some(0_i32), "exit code should be zero");
    }
    #[test]
    fn cli_keyboard_input_is_reflected_on_pty_screen() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_shell(&cwd, "powershell");
        let marker = "MCP_PTY_TYPED_INPUT";
        let written = write_keyboard(&created.shell_id, marker.as_bytes());
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        assert_eq!(String::from_utf8(written.stdout).unwrap().trim(), "ok");
        let query = wait_for_screen_contains(&created.shell_id, marker);
        assert_eq!(query.recognized_as, "shell", "query kind should be shell");
    }
    #[test]
    fn cli_keyboard_enter_runs_command_through_pty() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_shell(&cwd, "powershell");
        let marker = "MCP_PTY_KEYBOARD_EVENT";
        let command = format!("Write-Output '{marker}'\r\n");
        let written = write_keyboard(&created.shell_id, command.as_bytes());
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        wait_for_screen_contains(&created.shell_id, marker);
    }
    #[test]
    fn cli_query_reports_shell_liveness() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_shell(&cwd, "powershell");
        let alive_query =
            super::support::parse_shell_query(&run_cli(&["query", &created.shell_id]));
        assert!(alive_query.alive, "new shell should be alive");
        let _closed = super::support::send_command(&created.shell_id, "exit", 0.2);
        let mut dead_query =
            super::support::parse_shell_query(&run_cli(&["query", &created.shell_id]));
        for _attempt in 0_usize..20 {
            if !dead_query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
            dead_query = super::support::parse_shell_query(&run_cli(&["query", &created.shell_id]));
        }
        panic!("query should report exited shell as not alive");
    }
    #[test]
    fn cli_waiting_command_does_not_block_other_requests() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let first = create_shell(&cwd, "powershell");
        let second = create_shell(&cwd, "powershell");
        let first_shell_id = first.shell_id;
        let worker = thread::spawn(move || {
            super::support::send_command(
                &first_shell_id,
                "Start-Sleep -Seconds 5; Write-Output 'MCP_PTY_WAIT_DONE'",
                6.0,
            )
        });
        thread::sleep(Duration::from_millis(500));
        let start = Instant::now();
        let query = super::support::parse_shell_query(&run_cli(&["query", &second.shell_id]));
        let elapsed = start.elapsed();
        assert!(query.alive, "second shell should remain alive");
        assert!(
            elapsed < Duration::from_secs(2),
            "query should not wait for unrelated command; elapsed {elapsed:?}"
        );
        let accepted = worker.join().unwrap();
        let command_query = parse_command_query(&accepted);
        assert!(command_query.finished, "long command should finish");
        assert!(
            command_query.stdout.contains("MCP_PTY_WAIT_DONE"),
            "stdout should include long command marker"
        );
    }
    fn wait_for_screen_contains(shell_id: &str, expected: &str) -> super::support::ShellQuery {
        let mut last_screen = String::new();
        for _attempt in 0_usize..50 {
            let query = super::support::parse_shell_query(&run_cli(&["query", shell_id]));
            if query.screen.contains(expected) {
                return query;
            }
            last_screen = query.screen;
            thread::sleep(Duration::from_millis(100));
        }
        panic!("screen should contain {expected:?}; last screen:\n{last_screen}");
    }
}
