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
        ChildGuard, create_powershell_shell, locked, parse_command_accepted, parse_command_query,
        run_cli, run_cli_with_pipes, send_test_command,
    };
    use core::time::Duration;
    use std::process::{Command, Stdio};
    use std::thread;
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
    fn cli_creates_shell_with_short_id() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_powershell_shell(&cwd);
        assert_eq!(created.shell_id.len(), 12);
    }
    #[test]
    fn cli_send_command_returns_short_id() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_powershell_shell(&cwd);
        let accepted = parse_command_accepted(&send_test_command(&created.shell_id));
        assert_eq!(accepted.command_id.len(), 12);
    }
    #[test]
    fn cli_send_command_output_includes_command_snapshot() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_powershell_shell(&cwd);
        let query = parse_command_query(&send_test_command(&created.shell_id));
        assert_successful_test_query(&query);
    }
    #[test]
    fn cli_query_returns_command_output() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_powershell_shell(&cwd);
        let accepted = parse_command_accepted(&send_test_command(&created.shell_id));
        let query = parse_command_query(&run_cli(&["query", &accepted.command_id]));
        assert_successful_test_query(&query);
    }
    #[test]
    fn mcp_mode_starts_without_schema_panic() {
        let _guard = locked();
        let mut child = ChildGuard::new(
            Command::new(super::support::exe())
                .arg("mcp")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .unwrap(),
        );
        thread::sleep(Duration::from_secs(1));
        assert!(child.is_running());
    }
    fn assert_successful_test_query(query: &super::support::CommandQuery) {
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
}
