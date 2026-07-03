#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use crate::support::locked;
    #[cfg(not(windows))]
    use crate::support::locked_with_env;
    use crate::support::{
        create_tab, parse_command_result, parse_tab_view, run_cli_with_env, send_command,
    };
    use std::thread;
    #[test]
    fn daemon_serves_parallel_clients_without_test_serialisation() {
        let guard = system_shell_daemon();
        let tab = create_tab(&std::env::temp_dir(), system_shell_name());
        let env = guard.env();
        let mut workers = Vec::new();
        for _index in 0_usize..4 {
            let worker_env = env.clone();
            let tab_id = tab.tab_id.clone();
            workers.push(thread::spawn(move || {
                run_cli_with_env(&["view", &tab_id], &worker_env)
            }));
        }
        for worker in workers {
            let view = parse_tab_view(&worker.join().unwrap());
            assert!(view.alive, "parallel view should report a live tab");
        }
    }
    #[test]
    fn daemon_republishes_endpoint_after_restart() {
        let mut guard = system_shell_daemon();
        guard.restart_daemon();
        let tab = create_tab(&std::env::temp_dir(), system_shell_name());
        let view = parse_tab_view(&run_cli_with_env(&["view", &tab.tab_id], &guard.env()));
        assert!(view.alive, "restarted daemon should accept clients");
    }
    #[test]
    fn command_finishes_when_wrapper_command_directory_is_removed() {
        let _guard = system_shell_daemon();
        let tab = create_tab(&std::env::temp_dir(), system_shell_name());
        let command = delete_command_directory();
        let output = send_command(&tab.tab_id, command, 5.0);
        let result = parse_command_result(&output);
        assert!(result.finished, "wrapper should still publish done.json");
        assert_eq!(result.exit_code, Some(expected_delete_exit_code()));
    }
    #[cfg(windows)]
    fn system_shell_daemon() -> crate::support::TestGuard {
        locked()
    }
    #[cfg(not(windows))]
    fn system_shell_daemon() -> crate::support::TestGuard {
        let bash = required_executable("bash");
        locked_with_env(&[("FUNCTERM_BASH", &bash)])
    }
    #[cfg(windows)]
    const fn system_shell_name() -> &'static str {
        "powershell"
    }
    #[cfg(not(windows))]
    const fn system_shell_name() -> &'static str {
        "bash"
    }
    #[cfg(windows)]
    const fn delete_command_directory() -> &'static str {
        "Remove-Item -LiteralPath $env:FUNCTERM_COMMAND_DIRECTORY -Recurse -Force; cmd /c exit 7"
    }
    #[cfg(not(windows))]
    const fn delete_command_directory() -> &'static str {
        "rm -rf \"$FUNCTERM_COMMAND_DIRECTORY\"; false"
    }
    #[cfg(windows)]
    const fn expected_delete_exit_code() -> i32 {
        7
    }
    #[cfg(not(windows))]
    const fn expected_delete_exit_code() -> i32 {
        1
    }
    #[cfg(not(windows))]
    fn required_executable(name: &str) -> String {
        which::which(name)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| panic!("CI on Unix must install required shell executable {name}"))
    }
}
