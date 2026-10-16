#[cfg(windows)]
use crate::support::locked;
use crate::support::{
    create_tab, create_tab_with_env, locked_with_env, parse_command_result, parse_tab_created,
    parse_tab_view, run_cli_with_env, send_command, temp_dir, temp_root,
};
use std::path::Path;
#[test]
fn daemon_accepts_clients_after_restart() {
    let mut guard = system_shell_daemon();
    let previous = create_tab(&temp_root(), system_shell_name());
    let initial = parse_command_result(&send_command(&previous.tab_id, "echo BEFORE_RESTART", 5.0));
    assert_eq!(initial.exit_code, Some(0_i32));
    assert!(initial.stdout.contains("BEFORE_RESTART"));
    let duplicate = run_cli_with_env(&["daemon", "start"], &guard.env());
    assert!(duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stdout).contains("daemon is already running"));
    guard.restart_daemon();
    let tab = create_tab(&temp_root(), system_shell_name());
    let view = parse_tab_view(&run_cli_with_env(&["view", &tab.tab_id], &guard.env()));
    assert!(view.alive, "restarted daemon should accept clients");
    let recovered = parse_command_result(&send_command(&tab.tab_id, "echo AFTER_RESTART", 5.0));
    assert_eq!(recovered.exit_code, Some(0_i32));
    assert!(recovered.stdout.contains("AFTER_RESTART"));
    drop(guard);
}
#[test]
fn independent_test_processes_keep_their_runtime() {
    const CHILD_ENV: &str = "FUNCTERM_ISOLATION_PROBE_CHILD";
    let _guard = system_shell_daemon();
    let tab = create_tab(&temp_root(), system_shell_name());
    let initial = parse_command_result(&send_command(&tab.tab_id, "echo ISOLATION_BEGIN", 5.0));
    assert_eq!(initial.exit_code, Some(0_i32));
    if std::env::var_os(CHILD_ENV).is_some() {
        return;
    }
    let child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "services::recovery::independent_test_processes_keep_their_runtime",
            "--nocapture",
        ])
        .env(CHILD_ENV, "1")
        .output()
        .unwrap();
    assert!(
        child.status.success(),
        "independent test process failed: {child:?}"
    );
    let continued =
        parse_command_result(&send_command(&tab.tab_id, "echo ISOLATION_CONTINUED", 5.0));
    assert_eq!(continued.exit_code, Some(0_i32));
    assert!(continued.stdout.contains("ISOLATION_CONTINUED"));
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
    let recovered = parse_command_result(&send_command(&tab.tab_id, "echo RECORD_RECOVERED", 5.0));
    assert_eq!(recovered.exit_code, Some(0_i32));
    assert!(recovered.stdout.contains("RECORD_RECOVERED"));
}
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
    let output = send_command(&tab.tab_id, "functerm-client-path-probe", 5.0);
    let result = parse_command_result(&output);
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32));
    assert!(result.stdout.contains("FUNCTERM_CLIENT_PATH_PROBE"));
    drop(guard);
}
#[test]
fn new_tab_inherits_environment_from_each_client() {
    let guard = locked_with_env(&[
        ("FUNCTERM_CLIENT_ENV_PROBE", "daemon environment"),
        ("FUNCTERM_DAEMON_ONLY_PROBE", "daemon only"),
    ]);
    let base_environment = guard
        .env()
        .into_iter()
        .filter(|pair| {
            pair.0 != "FUNCTERM_CLIENT_ENV_PROBE" && pair.0 != "FUNCTERM_DAEMON_ONLY_PROBE"
        })
        .collect::<Vec<_>>();
    for expected in ["first client = 中文", "second client = 独立", ""] {
        let mut client_environment = base_environment.clone();
        if !expected.is_empty() {
            client_environment.push(("FUNCTERM_CLIENT_ENV_PROBE".to_owned(), expected.to_owned()));
        }
        let tab = parse_tab_created(&run_cli_with_env(
            &[
                "new-tab",
                "--starting-directory",
                temp_root().to_str().unwrap(),
                "--starting-shell",
                system_shell_name(),
                "--no-profile",
            ],
            &client_environment,
        ));
        let command = environment_probe_command(expected);
        let output = send_command(&tab.tab_id, &command, 5.0);
        let result = parse_command_result(&output);
        assert!(result.finished, "command should finish: {}", result.stderr);
        assert_eq!(result.exit_code, Some(0_i32), "stderr: {}", result.stderr);
        assert!(result.stdout.contains("CLIENT_ENVIRONMENT_INHERITED"));
    }
    drop(guard);
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
#[cfg(windows)]
fn write_path_probe(directory: &Path) {
    std::fs::write(
        directory.join("functerm-client-path-probe.cmd"),
        "@echo FUNCTERM_CLIENT_PATH_PROBE\r\n",
    )
    .unwrap();
}
#[cfg(windows)]
fn environment_probe_command(expected: &str) -> String {
    format!(
        "if (Test-Path Env:FUNCTERM_DAEMON_ONLY_PROBE) {{ throw 'daemon environment leaked' }}\nif ([string]$env:FUNCTERM_CLIENT_ENV_PROBE -cne {}) {{ throw ('client environment mismatch; actual UTF-8 base64: ' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes([string]$env:FUNCTERM_CLIENT_ENV_PROBE))) }}\nWrite-Output CLIENT_ENVIRONMENT_INHERITED",
        functerm::shell::quote::powershell_string(expected),
    )
}
#[cfg(not(windows))]
fn environment_probe_command(expected: &str) -> String {
    format!(
        "if [[ -v FUNCTERM_DAEMON_ONLY_PROBE ]]; then\n    exit 2\nfi\nif [[ \"${{FUNCTERM_CLIENT_ENV_PROBE-}}\" != {} ]]; then\n    exit 3\nfi\nprintf '%s\\n' CLIENT_ENVIRONMENT_INHERITED",
        functerm::shell::quote::posix_string(expected),
    )
}
#[cfg(not(windows))]
fn required_executable(name: &str) -> String {
    crate::support::required_executable(&[name])
        .to_string_lossy()
        .into_owned()
}
