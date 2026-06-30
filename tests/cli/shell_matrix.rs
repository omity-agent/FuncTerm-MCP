#[cfg(test)]
mod tests {
    use crate::support::{
        create_shell, locked, locked_with_env, parse_command_query, parse_shell_query, run_cli,
        send_command,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    struct ShellCase {
        name: &'static str,
        env_var: &'static str,
        expected_exit_code: i32,
    }
    #[test]
    fn cli_runs_commands_for_every_supported_shell() {
        let _guard = locked();
        for case in shell_cases() {
            let start = case_dir(case.name, "start dir");
            let next = case_dir(case.name, "next dir");
            let created = create_shell(&start, case.name);
            let shell_before = parse_shell_query(&run_cli(&["query", &created.shell_id]));
            assert_shell_query(&shell_before, &start, case.name);
            let command = case_command(case.name, &next);
            let command_query =
                parse_command_query(&send_command(&created.shell_id, &command, 10_000));
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
            let shell_after = parse_shell_query(&run_cli(&["query", &created.shell_id]));
            assert_shell_query(&shell_after, &next, case.name);
        }
    }
    #[test]
    fn cli_reports_startup_failure_for_every_supported_shell() {
        for case in shell_cases() {
            let output = {
                let _guard = locked_with_env(&[(case.env_var, "where.exe")]);
                let cwd = std::env::temp_dir();
                run_cli(&[
                    "new-shell",
                    "--cwd",
                    cwd.to_str().unwrap(),
                    "--shell",
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
    const fn shell_cases() -> [ShellCase; 4] {
        [
            ShellCase {
                name: "powershell",
                env_var: "SHELL_MCP_PTY_WINDOWS_POWERSHELL",
                expected_exit_code: 7,
            },
            ShellCase {
                name: "pwsh",
                env_var: "SHELL_MCP_PTY_PWSH",
                expected_exit_code: 7,
            },
            ShellCase {
                name: "bash",
                env_var: "SHELL_MCP_PTY_BASH",
                expected_exit_code: 1,
            },
            ShellCase {
                name: "nu",
                env_var: "SHELL_MCP_PTY_NUSHELL",
                expected_exit_code: 0,
            },
        ]
    }
    fn case_dir(shell: &str, leaf: &str) -> PathBuf {
        let path = std::env::temp_dir()
            .join("agent")
            .join("shell-mcp-cli")
            .join(format!("{shell}-{}", uuid::Uuid::new_v4().simple()))
            .join("quote ' segment")
            .join(leaf);
        fs::create_dir_all(&path).unwrap();
        path
    }
    fn case_command(shell: &str, next: &Path) -> String {
        match shell {
            "powershell" | "pwsh" => format!(
                "Write-Output 'MCP_PTY_STDOUT'; Write-Error 'MCP_PTY_STDERR'; Set-Location -LiteralPath {}; cmd /c exit 7",
                ps_quote(next)
            ),
            "bash" => format!(
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
    fn assert_shell_query(query: &crate::support::ShellQuery, cwd: &Path, shell: &str) {
        assert_eq!(
            query.recognized_as, "shell",
            "{shell} query kind should be shell"
        );
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
