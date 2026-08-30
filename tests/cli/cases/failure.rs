#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use crate::support::locked;
    #[cfg(not(windows))]
    use crate::support::locked_with_env;
    #[cfg(not(windows))]
    use crate::support::temp_dir;
    use crate::support::{
        create_tab, create_tab_with_env, parse_command_result, parse_tab_view, run_cli_with_env,
        send_command, temp_root,
    };
    #[cfg(not(windows))]
    use std::path::Path;
    use std::thread;
    #[test]
    fn daemon_serves_parallel_clients_without_test_serialisation() {
        let guard = system_shell_daemon();
        let tab = create_tab(&temp_root(), system_shell_name());
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
    fn daemon_accepts_clients_after_restart() {
        let mut guard = system_shell_daemon();
        guard.restart_daemon();
        let tab = create_tab(&temp_root(), system_shell_name());
        let view = parse_tab_view(&run_cli_with_env(&["view", &tab.tab_id], &guard.env()));
        assert!(view.alive, "restarted daemon should accept clients");
    }
    #[test]
    fn command_finishes_when_wrapper_command_directory_is_removed() {
        let _guard = system_shell_daemon();
        let tab = create_tab(&temp_root(), system_shell_name());
        let command = delete_command_directory();
        let output = send_command(&tab.tab_id, command, 5.0);
        let result = parse_command_result(&output);
        assert!(result.finished, "wrapper should still publish done.json");
        assert_eq!(result.exit_code, Some(expected_delete_exit_code()));
    }
    #[cfg(not(windows))]
    #[test]
    fn new_tab_uses_client_path_after_daemon_startup() {
        let guard = system_shell_daemon();
        let probe_dir = temp_dir("client-path-probe");
        write_path_probe(&probe_dir);
        let inherited_path = std::env::var_os("PATH").unwrap();
        let client_path = std::env::join_paths(
            core::iter::once(probe_dir).chain(std::env::split_paths(&inherited_path)),
        )
        .unwrap()
        .into_string()
        .unwrap();
        let mut client_env = guard.env();
        client_env.push(("PATH".to_owned(), client_path));
        let tab = create_tab_with_env(&temp_root(), system_shell_name(), &client_env);
        let output = send_command(&tab.tab_id, path_probe_command(), 5.0);
        let result = parse_command_result(&output);
        assert!(result.finished);
        assert_eq!(result.exit_code, Some(0_i32));
        assert!(result.stdout.contains("FUNCTERM_CLIENT_PATH_PROBE"));
    }
    #[cfg(windows)]
    #[test]
    fn new_tab_uses_fresh_user_environment() {
        let guard = locked();
        let mut client_env = guard.env();
        client_env.push((
            "FUNCTERM_CLIENT_ENV_PROBE".to_owned(),
            "must-not-be-inherited".to_owned(),
        ));
        let tab = create_tab_with_env(&temp_root(), "powershell", &client_env);
        let output = send_command(
            &tab.tab_id,
            "if ($null -ne $env:FUNCTERM_CLIENT_ENV_PROBE) { exit 2 }; if ([string]::IsNullOrEmpty($env:SystemRoot) -or [string]::IsNullOrEmpty($env:USERPROFILE)) { exit 3 }; Write-Output 'FUNCTERM_FRESH_ENVIRONMENT'",
            5.0,
        );
        let result = parse_command_result(&output);
        assert!(result.finished, "command should finish: {}", result.stderr);
        assert_eq!(result.exit_code, Some(0_i32), "stderr: {}", result.stderr);
        assert!(result.stdout.contains("FUNCTERM_FRESH_ENVIRONMENT"));
    }
    #[cfg(windows)]
    #[test]
    fn powershell_runs_internal_helpers_when_client_pathext_excludes_exe() {
        let guard = locked();
        let mut client_env = guard.env();
        client_env.push(("PATHEXT".to_owned(), ".CPL".to_owned()));
        let tab = create_tab_with_env(&temp_root(), "powershell", &client_env);
        let output = send_command(
            &tab.tab_id,
            "Write-Output 'FUNCTERM_RESTRICTED_PATHEXT_PROBE'",
            5.0,
        );
        let result = parse_command_result(&output);
        assert!(result.finished, "command should finish: {}", result.stderr);
        assert_eq!(result.exit_code, Some(0_i32), "stderr: {}", result.stderr);
        assert!(
            result.stdout.contains("FUNCTERM_RESTRICTED_PATHEXT_PROBE"),
            "stdout: {}; stderr: {}",
            result.stdout,
            result.stderr
        );
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
    fn write_path_probe(directory: &Path) {
        use std::os::unix::fs::PermissionsExt as _;
        let path = directory.join("functerm-client-path-probe");
        std::fs::write(&path, "#!/bin/sh\nprintf 'FUNCTERM_CLIENT_PATH_PROBE\\n'\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[cfg(not(windows))]
    const fn path_probe_command() -> &'static str {
        "functerm-client-path-probe"
    }
    #[cfg(not(windows))]
    fn required_executable(name: &str) -> String {
        crate::support::required_executable(&[name])
            .to_string_lossy()
            .into_owned()
    }
}
