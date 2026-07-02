#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, locked_with_env, manual_write, parse_command_query, parse_tab_query, run_cli,
        send_command,
    };
    use core::sync::atomic::{AtomicU64, Ordering};
    use std::fs;
    use std::path::{Path, PathBuf};
    static CASE_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);
    struct ShellCase {
        name: &'static str,
        env_var: &'static str,
        executables: &'static [&'static str],
        expected_exit_code: i32,
    }
    #[test]
    fn cli_runs_commands_for_every_supported_shell() {
        for case in shell_cases() {
            if let Some(executable) = available_executable(case.executables) {
                let _guard = locked_with_env(&[(case.env_var, &executable)]);
                let start = case_dir(case.name, "start dir");
                let next = case_dir(case.name, "next dir");
                let created = create_tab(&start, case.name);
                let shell_before = parse_tab_query(&run_cli(&["query", &created.tab_id]));
                assert_shell_query(&shell_before, &start, case.name);
                let command = case_command(case.name, &next);
                let command_query =
                    parse_command_query(&send_command(&created.tab_id, &command, 10.0));
                assert_eq!(command_query.recognized_as, "command");
                assert!(
                    command_query.finished,
                    "{name} command should finish",
                    name = case.name
                );
                assert!(
                    command_query.stdout.contains("MCP_PTY_STDOUT"),
                    "{name} stdout should include marker: {stdout}",
                    name = case.name,
                    stdout = command_query.stdout
                );
                assert!(
                    command_query.stderr.contains("MCP_PTY_STDERR"),
                    "{name} stderr should include marker: {stderr}",
                    name = case.name,
                    stderr = command_query.stderr
                );
                assert_eq!(
                    command_query.exit_code,
                    Some(case.expected_exit_code),
                    "{name} exit code should be captured",
                    name = case.name
                );
                assert_cwd(&command_query.cwd, &next, case.name);
                let shell_after = parse_tab_query(&run_cli(&["query", &created.tab_id]));
                assert_shell_query(&shell_after, &next, case.name);
            }
        }
    }
    #[test]
    fn cli_reports_startup_failure_for_every_supported_shell() {
        for case in shell_cases() {
            let output = {
                let _guard = locked_with_env(&[(case.env_var, immediately_exiting_executable())]);
                let cwd = std::env::temp_dir();
                run_cli(&[
                    "new-tab",
                    "--starting-directory",
                    cwd.to_str().unwrap(),
                    "--starting-shell",
                    case.name,
                ])
            };
            assert!(
                !output.status.success(),
                "{name} should fail startup",
                name = case.name
            );
            assert!(
                String::from_utf8_lossy(&output.stderr).contains("startup"),
                "{name} stderr should explain startup failure: {stderr}",
                name = case.name,
                stderr = String::from_utf8_lossy(&output.stderr)
            );
        }
    }
    #[test]
    fn cli_keeps_send_command_available_after_nested_shell_launch() {
        for case in shell_cases() {
            if let Some(executable) = available_executable(case.executables) {
                let _guard = locked_with_env(&[(case.env_var, &executable)]);
                let start = case_dir(case.name, "nested start");
                let created = create_tab(&start, case.name);
                let launch = parse_command_query(&send_command(
                    &created.tab_id,
                    nested_launch_command(case.name),
                    10.0,
                ));
                assert!(
                    launch.finished,
                    "{name} nested launch should finish early",
                    name = case.name
                );
                assert_eq!(launch.exit_code, Some(0_i32));
                let marker = format!("MCP_PTY_NESTED_{}", case.name.to_ascii_uppercase());
                let nested = parse_command_query(&send_command(
                    &created.tab_id,
                    &nested_marker_command(case.name, &marker),
                    10.0,
                ));
                assert!(
                    nested.finished,
                    "{name} command in nested shell should finish",
                    name = case.name
                );
                assert!(
                    nested.stdout.contains(&marker),
                    "{name} nested stdout should include marker: {stdout}",
                    name = case.name,
                    stdout = nested.stdout
                );
                let _closed = manual_write(&created.tab_id, b"exit\n");
            }
        }
    }
    const fn shell_cases() -> [ShellCase; 4] {
        [
            ShellCase {
                name: "powershell",
                env_var: "SHELL_MCP_PTY_POWERSHELL",
                executables: &["pwsh", "pwsh.exe", "powershell", "powershell.exe"],
                expected_exit_code: 7,
            },
            ShellCase {
                name: "bash",
                env_var: "SHELL_MCP_PTY_BASH",
                executables: &["bash", "bash.exe"],
                expected_exit_code: 1,
            },
            ShellCase {
                name: "nu",
                env_var: "SHELL_MCP_PTY_NUSHELL",
                executables: &["nu", "nu.exe"],
                expected_exit_code: 0,
            },
            ShellCase {
                name: "zsh",
                env_var: "SHELL_MCP_PTY_ZSH",
                executables: &["zsh"],
                expected_exit_code: 1,
            },
        ]
    }
    fn available_executable(executables: &[&str]) -> Option<String> {
        executables
            .iter()
            .find_map(|executable| which::which(executable).ok())
            .map(|path| path.to_string_lossy().into_owned())
    }
    #[cfg(windows)]
    fn immediately_exiting_executable() -> &'static str {
        "where.exe"
    }
    #[cfg(not(windows))]
    fn immediately_exiting_executable() -> &'static str {
        "false"
    }
    fn case_dir(shell: &str, leaf: &str) -> PathBuf {
        let unique = CASE_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join("shell-mcp-cli")
            .join(format!("{shell}-{}-{unique}", std::process::id()))
            .join("quote ' segment")
            .join(leaf);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn case_command(shell: &str, next: &Path) -> String {
        match shell {
            "powershell" => format!(
                "Write-Output 'MCP_PTY_STDOUT'; Write-Error 'MCP_PTY_STDERR'; Set-Location -LiteralPath {}; cmd /c exit 7",
                ps_quote(next)
            ),
            "bash" | "zsh" => format!(
                "printf 'MCP_PTY_STDOUT\\n'; printf 'MCP_PTY_STDERR\\n' >&2; cd {}; false",
                sh_quote(&bash_path(next))
            ),
            "nu" => format!(
                "print 'MCP_PTY_STDOUT'; print --stderr 'MCP_PTY_STDERR'; cd {}",
                nu_quote(&next.to_string_lossy())
            ),
            other => panic!("unsupported shell case {other}"),
        }
    }
    fn nested_launch_command(shell: &str) -> &'static str {
        match shell {
            "powershell" => "pwsh",
            "bash" => "bash",
            "nu" => "nu",
            "zsh" => "zsh",
            other => panic!("unsupported shell case {other}"),
        }
    }
    fn nested_marker_command(shell: &str, marker: &str) -> String {
        match shell {
            "powershell" => format!("Write-Output '{marker}'"),
            "bash" | "zsh" => format!("printf '{}\\n'", marker.replace('\'', "'\\''")),
            "nu" => format!("print '{}'", marker.replace('\'', "\\'")),
            other => panic!("unsupported shell case {other}"),
        }
    }
    fn assert_shell_query(query: &crate::support::TabQuery, cwd: &Path, shell: &str) {
        assert_eq!(
            query.recognized_as, "tab",
            "{shell} query kind should be tab"
        );
        assert!(query.alive, "{shell} query should report live shell");
        assert_cwd(&query.cwd, cwd, shell);
        assert!(
            !query.screen.is_empty(),
            "{shell} screen should be reported"
        );
    }
    fn assert_cwd(actual: &str, expected: &Path, shell: &str) {
        let leaf = expected.file_name().unwrap().to_string_lossy();
        assert!(
            actual.replace('\\', "/").contains(&leaf.replace('\\', "/")),
            "{shell} cwd should include {}, got {actual}",
            expected.display()
        );
    }
    fn ps_quote(path: &Path) -> String {
        let text = path.to_string_lossy().replace('\'', "''");
        format!("'{text}'")
    }
    fn bash_path(path: &Path) -> String {
        path.to_string_lossy().replace('\\', "/")
    }
    fn sh_quote(value: &str) -> String {
        format!("'{}'", value.replace('\'', "'\\''"))
    }
    fn nu_quote(value: &str) -> String {
        let text = value.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{text}\"")
    }
}
