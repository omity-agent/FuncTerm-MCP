use super::daemon;
use super::parse::{TabCreated, parse_tab_created};
use super::process::read_pipe;
use super::process::wait_for_status;
use base64_turbo::STANDARD;
use core::time::Duration;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
pub(crate) const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const PIPE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_functerm"))
}
pub(crate) fn run_cli(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    daemon::apply_active_env(&mut command);
    output_from_pipes(command, CLI_COMMAND_TIMEOUT)
}
pub(crate) fn run_cli_with_env(arguments: &[&str], env: &[(String, String)]) -> Output {
    let previous = daemon::active_env();
    daemon::set_active_env(env);
    let output = run_cli(arguments);
    daemon::set_active_env(&previous);
    output
}
pub(crate) fn create_tab(cwd: &Path, shell: &str) -> TabCreated {
    create_tab_with_env(cwd, shell, &daemon::active_env())
}
pub(crate) fn create_tab_with_env(cwd: &Path, shell: &str, env: &[(String, String)]) -> TabCreated {
    parse_tab_created(&run_cli_with_env(
        &[
            "new-tab",
            "--no-profile",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            shell,
        ],
        env,
    ))
}
pub(crate) fn send_command(tab_id: &str, command: &str, wait_timeout: f64) -> Output {
    run_cli(&[
        "send-command",
        tab_id,
        "--command",
        command,
        "--separate-output",
        "true",
        "--wait-timeout",
        &wait_timeout.to_string(),
    ])
}
pub(crate) fn manual_write(tab_id: &str, bytes: &[u8], wait_timeout: f64) -> Output {
    let encoded = STANDARD.encode(bytes);
    run_cli(&[
        "manual-write",
        tab_id,
        "--base64",
        &encoded,
        "--wait-timeout",
        &wait_timeout.to_string(),
    ])
}
fn output_from_pipes(mut command: Command, timeout: Duration) -> Output {
    let description = format!("{command:?}");
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_rx = read_pipe(stdout);
    let stderr_rx = read_pipe(stderr);
    let status = wait_for_status(&mut child, timeout, &description);
    Output {
        status,
        stdout: stdout_rx
            .recv_timeout(PIPE_CLOSE_TIMEOUT)
            .unwrap_or_else(|error| panic!("stdout did not close after {description}: {error}")),
        stderr: stderr_rx
            .recv_timeout(PIPE_CLOSE_TIMEOUT)
            .unwrap_or_else(|error| panic!("stderr did not close after {description}: {error}")),
    }
}
