#[cfg(test)]
#[cfg(windows)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use core::time::Duration;
    use serde::Deserialize;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::sync::Mutex;
    use std::thread;
    static CLI_TEST_LOCK: Mutex<()> = Mutex::new(());
    struct ChildGuard {
        child: Child,
    }
    impl ChildGuard {
        fn new(child: Child) -> Self {
            Self { child }
        }
        fn is_running(&mut self) -> bool {
            self.child.try_wait().unwrap().is_none()
        }
    }
    #[expect(
        clippy::missing_trait_methods,
        reason = "Drop only needs the regular destructor for this test guard"
    )]
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if self.child.try_wait().unwrap().is_none() {
                self.child.kill().unwrap();
            }
            self.child.wait().unwrap();
        }
    }
    #[derive(Deserialize)]
    struct ShellCreated {
        kind: String,
        shell_id: String,
    }
    #[derive(Deserialize)]
    struct CommandAccepted {
        kind: String,
        command_id: String,
        end_reason: String,
    }
    #[derive(Deserialize)]
    struct CommandQuery {
        kind: String,
        recognized_as: String,
        finished: bool,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    }
    fn exe() -> PathBuf {
        PathBuf::from(env!("CARGO_BIN_EXE_shell-mcp-pty"))
    }
    fn run_cli(arguments: &[&str]) -> std::process::Output {
        Command::new(exe()).args(arguments).output().unwrap()
    }
    fn parse_stdout<T>(output: &std::process::Output) -> T
    where
        T: for<'de> Deserialize<'de>,
    {
        assert!(
            output.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        sonic_rs::from_slice(&output.stdout).unwrap()
    }
    #[test]
    fn cli_rejects_missing_cwd() {
        let _guard = CLI_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
    fn cli_runs_command_after_creating_shell() {
        let _guard = CLI_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cwd = std::env::temp_dir();
        let created: ShellCreated = parse_stdout(&run_cli(&[
            "new-shell",
            "--cwd",
            cwd.to_str().unwrap(),
            "--shell",
            "powershell",
        ]));
        assert_eq!(created.kind, "shell_created");
        let accepted: CommandAccepted = parse_stdout(&run_cli(&[
            "send-command",
            &created.shell_id,
            "--command",
            "Write-Output 'MCP_PTY_TEST'",
            "--wait-ms",
            "5000",
        ]));
        assert_eq!(accepted.kind, "command_accepted");
        assert_eq!(accepted.end_reason, "command_ended");
        let query: CommandQuery = parse_stdout(&run_cli(&["query", &accepted.command_id]));
        assert_eq!(query.kind, "query");
        assert_eq!(query.recognized_as, "command");
        assert!(query.finished);
        assert!(query.stdout.contains("MCP_PTY_TEST"));
        assert_eq!(query.stderr, "");
        assert_eq!(query.exit_code, Some(0_i32));
    }
    #[test]
    fn mcp_mode_starts_without_schema_panic() {
        let _guard = CLI_TEST_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut child = ChildGuard::new(
            Command::new(exe())
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
}
