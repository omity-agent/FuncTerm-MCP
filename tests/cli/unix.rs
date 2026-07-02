#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, locked_with_env, manual_write, parse_command_id, parse_command_query,
        parse_tab_query, run_cli, send_command,
    };
    use core::time::Duration;
    use std::thread;
    use std::time::Instant;
    #[test]
    fn cli_rejects_missing_starting_directory_on_unix_shell() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let missing = std::env::temp_dir().join("definitely-missing-functerm-unix-cwd");
        let output = run_cli(&[
            "new-tab",
            "--starting-directory",
            missing.to_str().unwrap(),
            "--starting-shell",
            "bash",
        ]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("starting_directory does not exist or is not a directory")
        );
    }
    #[test]
    fn cli_keyboard_enter_runs_commands_on_unix_ptys() {
        for case in unix_keyboard_cases() {
            let Some(path) = executable(case.executable) else {
                continue;
            };
            let _guard = locked_with_env(&[(case.env_var, &path)]);
            let shell = create_tab(&std::env::temp_dir(), case.shell);
            let marker = format!("MCP_PTY_KEYBOARD_{}", case.shell);
            let command = keyboard_command(case.shell, &marker);
            let written = manual_write(&shell.tab_id, command.as_bytes());
            assert!(
                written.status.success(),
                "stdout: {}\nstderr: {}",
                String::from_utf8_lossy(&written.stdout),
                String::from_utf8_lossy(&written.stderr)
            );
            wait_for_screen_contains(&shell.tab_id, &marker);
        }
    }
    #[test]
    fn cli_view_reports_unix_shell_liveness_after_keyboard_exit() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let shell = create_tab(&std::env::temp_dir(), "bash");
        let alive = parse_tab_query(&run_cli(&["view", &shell.tab_id]));
        assert!(alive.alive);
        let written = manual_write(&shell.tab_id, b"exit\n");
        assert!(
            written.status.success(),
            "stdout: {}\nstderr: {}",
            String::from_utf8_lossy(&written.stdout),
            String::from_utf8_lossy(&written.stderr)
        );
        wait_for_shell_dead(&shell.tab_id);
    }
    #[test]
    fn cli_waiting_unix_command_does_not_block_other_requests() {
        let Some(bash) = executable("bash") else {
            return;
        };
        let _guard = locked_with_env(&[("SHELL_MCP_PTY_BASH", &bash)]);
        let first = create_tab(&std::env::temp_dir(), "bash");
        let second = create_tab(&std::env::temp_dir(), "bash");
        let first_tab_id = first.tab_id;
        let worker = thread::spawn(move || {
            send_command(&first_tab_id, "sleep 2; printf 'MCP_PTY_WAIT_DONE\\n'", 5.0)
        });
        thread::sleep(Duration::from_millis(300));
        let start = Instant::now();
        let query = parse_tab_query(&run_cli(&["view", &second.tab_id]));
        let elapsed = start.elapsed();
        assert!(query.alive);
        assert!(
            elapsed < Duration::from_secs(2),
            "view should not wait for unrelated command; elapsed {elapsed:?}"
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
        let shell = create_tab(&std::env::temp_dir(), "bash");
        let accepted = send_command(
            &shell.tab_id,
            "sleep 1; printf 'MCP_PTY_TIMEOUT_DONE\\n'",
            0.05,
        );
        let pending = parse_command_query(&accepted);
        assert!(!pending.finished);
        assert_eq!(pending.exit_code, None);
        let command_id = parse_command_id(&accepted);
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
    fn wait_for_screen_contains(tab_id: &str, expected: &str) {
        let mut last_screen = String::new();
        for _attempt in 0_usize..50 {
            let query = parse_tab_query(&run_cli(&["view", tab_id]));
            if query.screen.contains(expected) {
                return;
            }
            last_screen = query.screen;
            thread::sleep(Duration::from_millis(100));
        }
        panic!("screen should contain {expected:?}; last screen:\n{last_screen}");
    }
    fn wait_for_shell_dead(tab_id: &str) {
        for _attempt in 0_usize..30 {
            let query = parse_tab_query(&run_cli(&["view", tab_id]));
            if !query.alive {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("shell should be reported dead");
    }
    fn wait_for_command_finished(command_id: &str) -> crate::support::CommandQuery {
        for _attempt in 0_usize..30 {
            let query = parse_command_query(&run_cli(&["view", command_id]));
            if query.finished {
                return query;
            }
            thread::sleep(Duration::from_millis(100));
        }
        panic!("command {command_id} should finish");
    }
}
