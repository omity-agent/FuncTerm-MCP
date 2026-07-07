#[path = "cli/cases/failure.rs"]
mod failure;
#[path = "cli/cases/history.rs"]
mod history;
#[path = "cli/shell_matrix.rs"]
mod shell_matrix;
#[path = "cli/harness.rs"]
mod support;
#[cfg(windows)]
#[path = "cli/cases/tab_state.rs"]
mod tab_state;
#[cfg(unix)]
#[path = "cli/cases/unix.rs"]
mod unix;
#[cfg(windows)]
#[path = "cli/cases/windows_io.rs"]
mod windows_io;
#[cfg(windows)]
#[cfg(test)]
mod tests {
    use super::support::{
        create_tab, locked, manual_write, parse_command_id, parse_command_result, run_cli,
        send_command_with_env, temp_root,
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
    fn cli_manual_write_rejects_idle_prompt() {
        let _guard = locked();
        let cwd = temp_root();
        let created = create_tab(&cwd, "powershell");
        let marker = "MCP_PTY_TYPED_INPUT";
        let written = manual_write(&created.tab_id, marker.as_bytes());
        assert!(!written.status.success());
        assert!(
            String::from_utf8_lossy(&written.stderr).contains("prompt is idle"),
            "stderr: {}",
            String::from_utf8_lossy(&written.stderr)
        );
        let query = super::support::parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert!(query.alive, "rejected manual_write should keep tab alive");
    }
    #[test]
    fn cli_manual_write_feeds_running_powershell_command() {
        let _guard = locked();
        let cwd = temp_root();
        let created = create_tab(&cwd, "powershell");
        let marker = "MCP_PTY_KEYBOARD_EVENT";
        let accepted = super::support::send_command(
            &created.tab_id,
            "$line = [Console]::In.ReadLine(); Write-Output \"MCP_PTY_KEYBOARD_$line\"",
            0.0,
        );
        let pending = parse_command_result(&accepted);
        assert!(!pending.finished, "command should wait for manual input");
        let command_id = parse_command_id(&accepted);
        let typed = format!("{marker}\r\n");
        let written = manual_write(&created.tab_id, typed.as_bytes());
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        assert!(
            String::from_utf8_lossy(&written.stdout).contains("<SCREEN>\n"),
            "manual_write should return a screen snapshot"
        );
        let completed = wait_for_command_finished(&command_id);
        assert!(completed.stdout.contains(marker));
    }
    #[test]
    fn cli_manual_write_ctrl_c_interrupts_powershell_command() {
        let _guard = locked();
        let cwd = temp_root();
        let created = create_tab(&cwd, "powershell");
        let accepted = super::support::send_command(
            &created.tab_id,
            "while ($true) { Start-Sleep -Milliseconds 200 }",
            0.0,
        );
        let pending = parse_command_result(&accepted);
        assert!(
            !pending.finished,
            "command should keep running before Ctrl+C"
        );
        let command_id = parse_command_id(&accepted);
        let written = manual_write(&created.tab_id, &[3]);
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        let completed = wait_for_command_finished(&command_id);
        assert!(completed.finished, "Ctrl+C should finish the command");
    }
    #[test]
    fn cli_waiting_command_does_not_block_other_requests() {
        let guard = locked();
        let cwd = temp_root();
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
        let cwd = temp_root();
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
    fn wait_for_command_finished(command_id: &str) -> super::support::CommandResult {
        for _attempt in 0_usize..50 {
            let query = parse_command_result(&run_cli(&["view", command_id]));
            if query.finished {
                return query;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("command {command_id} should finish");
    }
}
