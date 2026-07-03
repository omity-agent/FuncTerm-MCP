#[path = "cli/failure.rs"]
mod failure;
#[path = "cli/history.rs"]
mod history;
#[path = "cli/shell_matrix.rs"]
mod shell_matrix;
#[path = "cli/support.rs"]
mod support;
#[cfg(windows)]
#[path = "cli/tab_state.rs"]
mod tab_state;
#[cfg(unix)]
#[path = "cli/unix.rs"]
mod unix;
#[cfg(windows)]
#[path = "cli/windows_io.rs"]
mod windows_io;
#[cfg(windows)]
#[cfg(test)]
mod tests {
    use super::support::{
        create_tab, locked, manual_write, parse_command_id, parse_command_result, run_cli,
        send_command_with_env, send_test_command,
    };
    use core::time::Duration;
    use std::thread;
    use std::time::Instant;
    #[test]
    fn cli_keeps_starting_directory_shell_syntax_literal() {
        let _guard = locked();
        let output = run_cli(&[
            "new-tab",
            "--starting-directory",
            "%FUNCTERM_TEST_STARTING_DIRECTORY%",
            "--starting-shell",
            "powershell",
        ]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("starting_directory does not exist or is not a directory")
        );
    }
    #[test]
    fn cli_daemon_refuses_second_instance() {
        let _guard = locked();
        let output = run_cli(&["daemon"]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("daemon is already running"),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    #[test]
    fn cli_view_returns_command_output() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let accepted_output = send_test_command(&created.tab_id);
        let query = parse_command_result(&accepted_output);
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
        let created = create_tab(&cwd, "powershell");
        let marker = "MCP_PTY_TYPED_INPUT";
        let written = manual_write(&created.tab_id, marker.as_bytes());
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        assert_eq!(
            String::from_utf8(written.stdout).unwrap().trim(),
            "<OK>\n\n</OK>"
        );
        let query = wait_for_screen_contains(&created.tab_id, marker);
        assert!(query.alive, "view should report live tab");
    }
    #[test]
    fn cli_keyboard_enter_runs_command_through_pty() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let marker = "MCP_PTY_KEYBOARD_EVENT";
        let command = format!("Write-Output '{marker}'\r\n");
        let written = manual_write(&created.tab_id, command.as_bytes());
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        wait_for_screen_contains(&created.tab_id, marker);
    }
    #[test]
    fn cli_view_reports_shell_liveness() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let alive_view = super::support::parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert!(alive_view.alive, "new tab should be alive");
        let _closed = super::support::send_command(&created.tab_id, "exit", 0.2);
        let mut dead_view = super::support::parse_tab_view(&run_cli(&["view", &created.tab_id]));
        for _attempt in 0_usize..20 {
            if !dead_view.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
            dead_view = super::support::parse_tab_view(&run_cli(&["view", &created.tab_id]));
        }
        panic!("view should report exited tab as not alive");
    }
    #[test]
    fn cli_waiting_command_does_not_block_other_requests() {
        let guard = locked();
        let cwd = std::env::temp_dir();
        let first = create_tab(&cwd, "powershell");
        let second = create_tab(&cwd, "powershell");
        let env = guard.env();
        let first_tab_id = first.tab_id;
        let worker = thread::spawn(move || {
            send_command_with_env(
                &env,
                &first_tab_id,
                "Start-Sleep -Seconds 5; Write-Output 'MCP_PTY_WAIT_DONE'",
                6.0,
            )
        });
        thread::sleep(Duration::from_millis(500));
        let start = Instant::now();
        let query = super::support::parse_tab_view(&run_cli(&["view", &second.tab_id]));
        let elapsed = start.elapsed();
        assert!(query.alive, "second tab should remain alive");
        assert!(
            elapsed < Duration::from_secs(2),
            "view should not wait for unrelated command; elapsed {elapsed:?}"
        );
        let accepted = worker.join().unwrap();
        let command_result = parse_command_result(&accepted);
        assert!(command_result.finished, "long command should finish");
        assert!(
            command_result.stdout.contains("MCP_PTY_WAIT_DONE"),
            "stdout should include long command marker"
        );
    }
    #[test]
    fn cli_view_waits_until_command_finishes() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let accepted = super::support::send_command(
            &created.tab_id,
            "Start-Sleep -Milliseconds 300; Write-Output 'MCP_PTY_VIEW_WAIT_DONE'",
            0.0,
        );
        let command_id = parse_command_id(&accepted);
        let start = Instant::now();
        let viewed = parse_command_result(&run_cli(&["view", &command_id, "--waiting", "5"]));
        let elapsed = start.elapsed();
        assert!(viewed.finished, "view should return after command finishes");
        assert!(
            elapsed < Duration::from_secs(2),
            "view should not wait for the full timeout after command completion; elapsed {elapsed:?}"
        );
        assert!(viewed.stdout.contains("MCP_PTY_VIEW_WAIT_DONE"));
    }
    fn wait_for_screen_contains(tab_id: &str, expected: &str) -> super::support::TabView {
        let mut last_screen = String::new();
        for _attempt in 0_usize..50 {
            let query = super::support::parse_tab_view(&run_cli(&["view", tab_id]));
            if query.screen.contains(expected) {
                return query;
            }
            last_screen = query.screen;
            thread::sleep(Duration::from_millis(100));
        }
        panic!("screen should contain {expected:?}; last screen:\n{last_screen}");
    }
}
