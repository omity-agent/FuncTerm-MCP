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
    };
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
            "pwsh",
        ]);
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("shell_id: "));
    }
    #[test]
    fn cli_query_returns_command_output() {
        let _guard = locked();
        let cwd = std::env::temp_dir();
        let created = create_shell(&cwd, "pwsh");
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
}
