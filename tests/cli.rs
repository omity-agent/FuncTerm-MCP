#[path = "cli/history.rs"]
mod history;
#[path = "cli/shell_matrix.rs"]
mod shell_matrix;
#[path = "cli/support.rs"]
mod support;
#[cfg(unix)]
#[path = "cli/unix.rs"]
mod unix;
#[cfg(windows)]
#[cfg(test)]
mod tests {
    use super::support::{
        create_tab, create_tab_from_directory_argument, locked, locked_with_env, manual_write,
        parse_command_id, parse_command_query, parse_tab_query, run_cli, run_cli_with_pipes,
        send_test_command,
    };
    use core::time::Duration;
    use std::thread;
    use std::time::Instant;
    #[test]
    fn cli_rejects_missing_starting_directory() {
        let _guard = locked();
        let missing = std::env::temp_dir().join("definitely-missing-mcp-pty-cli-cwd");
        let output = run_cli(&[
            "new-tab",
            "--starting-directory",
            missing.to_str().unwrap(),
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
    fn cli_pipe_capture_returns_without_hanging() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let output = run_cli_with_pipes(&[
            "new-tab",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            "powershell",
        ]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("<TAB_ID>"));
    }
    #[test]
    fn cli_expands_starting_directory_environment_variables() {
        let cwd = std::env::temp_dir();
        let cwd_text = cwd.to_str().unwrap();
        let _guard = locked_with_env(&[("FUNCTERM_TEST_STARTING_DIRECTORY", cwd_text)]);
        let created =
            create_tab_from_directory_argument("%FUNCTERM_TEST_STARTING_DIRECTORY%", "powershell");
        let query = parse_tab_query(&run_cli(&["view", &created.tab_id]));
        let actual = query.cwd.replace('\\', "/");
        let expected = cwd_text.replace('\\', "/");
        assert!(actual.contains(&expected));
    }
    #[test]
    fn cli_view_returns_command_output() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_tab(&cwd, "powershell");
        let accepted_output = send_test_command(&created.tab_id);
        let query = parse_command_query(&accepted_output);
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
        let alive_query = super::support::parse_tab_query(&run_cli(&["view", &created.tab_id]));
        assert!(alive_query.alive, "new tab should be alive");
        let _closed = super::support::send_command(&created.tab_id, "exit", 0.2);
        let mut dead_query = super::support::parse_tab_query(&run_cli(&["view", &created.tab_id]));
        for _attempt in 0_usize..20 {
            if !dead_query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
            dead_query = super::support::parse_tab_query(&run_cli(&["view", &created.tab_id]));
        }
        panic!("view should report exited tab as not alive");
    }
    #[test]
    fn cli_waiting_command_does_not_block_other_requests() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let first = create_tab(&cwd, "powershell");
        let second = create_tab(&cwd, "powershell");
        let first_tab_id = first.tab_id;
        let worker = thread::spawn(move || {
            super::support::send_command(
                &first_tab_id,
                "Start-Sleep -Seconds 5; Write-Output 'MCP_PTY_WAIT_DONE'",
                6.0,
            )
        });
        thread::sleep(Duration::from_millis(500));
        let start = Instant::now();
        let query = super::support::parse_tab_query(&run_cli(&["view", &second.tab_id]));
        let elapsed = start.elapsed();
        assert!(query.alive, "second tab should remain alive");
        assert!(
            elapsed < Duration::from_secs(2),
            "view should not wait for unrelated command; elapsed {elapsed:?}"
        );
        let accepted = worker.join().unwrap();
        let command_query = parse_command_query(&accepted);
        assert!(command_query.finished, "long command should finish");
        assert!(
            command_query.stdout.contains("MCP_PTY_WAIT_DONE"),
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
        let viewed = parse_command_query(&run_cli(&["view", &command_id, "--waiting", "5"]));
        let elapsed = start.elapsed();
        assert!(viewed.finished, "view should return after command finishes");
        assert!(
            elapsed < Duration::from_secs(2),
            "view should not wait for the full timeout after command completion; elapsed {elapsed:?}"
        );
        assert!(viewed.stdout.contains("MCP_PTY_VIEW_WAIT_DONE"));
    }
    fn wait_for_screen_contains(tab_id: &str, expected: &str) -> super::support::TabQuery {
        let mut last_screen = String::new();
        for _attempt in 0_usize..50 {
            let query = super::support::parse_tab_query(&run_cli(&["view", tab_id]));
            if query.screen.contains(expected) {
                return query;
            }
            last_screen = query.screen;
            thread::sleep(Duration::from_millis(100));
        }
        panic!("screen should contain {expected:?}; last screen:\n{last_screen}");
    }
}
