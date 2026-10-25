use crate::support::{
    create_tab_with_env, locked_with_env, parse_command_result, parse_tab_created,
    run_cli_with_env, send_command, temp_dir, temp_root,
};
use std::path::Path;
const SHELL: &str = if cfg!(windows) { "powershell" } else { "bash" };
#[test]
fn new_tab_uses_client_path_after_daemon_startup() {
    let guard = locked_with_env(&[]);
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
    let tab = create_tab_with_env(&temp_root(), SHELL, &client_env);
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
                SHELL,
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
