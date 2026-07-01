#[cfg(test)]
mod tests {
    use crate::support::{
        create_shell, locked_with_env, parse_command_query, parse_shell_query, run_cli,
        send_command, write_keyboard,
    };
    use core::time::Duration;
    use std::thread;
    use std::time::Instant;
    #[test]
    fn cli_rejects_missing_cwd_on_unix_shell() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let missing = std::env::temp_dir().join("definitely-missing-functerm-unix-cwd");
        let output = run_cli(&[
            "new-shell",
            "--cwd",
            missing.to_str().unwrap(),
            "--shell",
            "bash",
        ]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("cwd does not exist or is not a directory")
        );
    }
    #[test]
    fn cli_keyboard_enter_runs_commands_on_unix_ptys() {
        for case in unix_keyboard_cases() {
            let Some(path) = executable(case.executable) else {
                continue;
            };
            let _guard = locked_with_env(&[(case.env_var, &path)]);
            let shell = create_shell(&std::env::temp_dir(), case.shell);
            let marker = format!("MCP_PTY_KEYBOARD_{}", case.shell);
            let command = keyboard_command(case.shell, &marker);
            let written = write_keyboard(&shell.shell_id, command.as_bytes());
            assert!(
                written.status.success(),
                "stdout: {}\nstderr: {}",
                String::from_utf8_lossy(&written.stdout),
                String::from_utf8_lossy(&written.stderr)
            );
            wait_for_screen_contains(&shell.shell_id, &marker);
        }
    }
    #[test]
    fn cli_query_reports_unix_shell_liveness_after_keyboard_exit() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let shell = create_shell(&std::env::temp_dir(), "bash");
        let alive = parse_shell_query(&run_cli(&["query", &shell.shell_id]));
        assert!(alive.alive);
        let written = write_keyboard(&shell.shell_id, b"exit\n");
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        wait_for_shell_dead(&shell.shell_id);
    }
    #[test]
    fn cli_waiting_unix_command_does_not_block_other_requests() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let first = create_shell(&std::env::temp_dir(), "bash");
        let second = create_shell(&std::env::temp_dir(), "bash");
        let first_shell_id = first.shell_id;
        let worker = thread::spawn(move || {
            send_command(
                &first_shell_id,
                "sleep 2; printf 'MCP_PTY_WAIT_DONE\\n'",
                5.0,
            )
        });
        thread::sleep(Duration::from_millis(300));
        let start = Instant::now();
        let query = parse_shell_query(&run_cli(&["query", &second.shell_id]));
        let elapsed = start.elapsed();
        assert!(query.alive);
        assert!(
            elapsed < Duration::from_secs(2),
            "query should not wait for unrelated command; elapsed {elapsed:?}"
        );
        let accepted = worker.join().unwrap();
        let command_query = parse_command_query(&accepted);
        assert!(command_query.finished);
        assert!(command_query.stdout.contains("MCP_PTY_WAIT_DONE"));
    }
    #[test]
    fn cli_unix_command_timeout_can_be_queried_after_completion() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let shell = create_shell(&std::env::temp_dir(), "bash");
        let accepted = send_command(
            &shell.shell_id,
            "sleep 1; printf 'MCP_PTY_TIMEOUT_DONE\\n'",
            0.05,
        );
        let pending = parse_command_query(&accepted);
        assert!(!pending.finished);
        assert_eq!(pending.exit_code, None);
        let command_id = command_id(&accepted);
        let completed = wait_for_command_finished(&command_id);
        assert!(completed.stdout.contains("MCP_PTY_TIMEOUT_DONE"));
        assert_eq!(completed.exit_code, Some(0));
    }
    struct KeyboardCase {
        shell: &'static str,
        executable: &'static str,
        env_var: &'static str,
    }
    const fn unix_keyboard_cases() -> [KeyboardCase; 2] {
        [
            KeyboardCase {
                shell: "bash",
                executable: "bash",
                env_var: "SHELL_MCP_PTY_BASH",
            },
            KeyboardCase {
                shell: "zsh",
                executable: "zsh",
                env_var: "SHELL_MCP_PTY_ZSH",
            },
        ]
    }
    fn keyboard_command(shell: &str, marker: &str) -> String {
        match shell {
            "bash" => format!("printf '{marker}\\n'\n"),
            "zsh" => format!("print -r -- '{marker}'\n"),
            other => panic!("unsupported shell {other}"),
        }
    }
    fn executable(name: &str) -> Option<String> {
        which::which(name)
            .ok()
            .map(|path| path.to_string_lossy().into_owned())
    }
    fn wait_for_screen_contains(shell_id: &str, expected: &str) {
        let mut last_screen = String::new();
        for _attempt in 0_usize..50 {
            let query = parse_shell_query(&run_cli(&["query", shell_id]));
            if query.screen.contains(expected) {
                return;
            }
            last_screen = query.screen;
            thread::sleep(Duration::from_millis(100));
        }
        panic!("screen should contain {expected:?}; last screen:\n{last_screen}");
    }
    fn wait_for_shell_dead(shell_id: &str) {
        for _attempt in 0_usize..30 {
            let query = parse_shell_query(&run_cli(&["query", shell_id]));
            if !query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("shell should be reported dead");
    }
    fn wait_for_command_finished(command_id: &str) -> crate::support::CommandQuery {
        for _attempt in 0_usize..30 {
            let query = parse_command_query(&run_cli(&["query", command_id]));
            if query.finished {
                return query;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("command {command_id} should finish");
    }
    fn command_id(output: &std::process::Output) -> String {
        let text = String::from_utf8(output.stdout.clone()).unwrap();
        field(&text, "command_id")
    }
    fn field(text: &str, name: &str) -> String {
        let prefix = format!("{name}: ");
        text.lines()
            .find_map(|line| line.strip_prefix(&prefix).map(str::to_owned))
            .unwrap()
    }
}
