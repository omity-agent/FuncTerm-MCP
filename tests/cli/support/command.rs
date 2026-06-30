use super::daemon;
use super::parse::{ShellCreated, parse_shell_created};
use super::process::{output_from_parts, read_pipe, wait_for_status};
use core::time::Duration;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
pub(crate) const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const PIPE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_shell-mcp-pty"))
}
pub(crate) fn run_cli(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command.args(arguments).stdin(Stdio::null());
    daemon::apply_active_env(&mut command);
    output_to_files(command, CLI_COMMAND_TIMEOUT)
}
pub(crate) fn run_cli_with_pipes(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    daemon::apply_active_env(&mut command);
    output_from_pipes(command, CLI_COMMAND_TIMEOUT)
}
pub(crate) fn create_shell(cwd: &Path, shell: &str) -> ShellCreated {
    parse_shell_created(&run_cli(&[
        "new-shell",
        "--cwd",
        cwd.to_str().unwrap(),
        "--shell",
        shell,
    ]))
}
pub(crate) fn send_command(shell_id: &str, command: &str, wait_ms: u64) -> Output {
    run_cli(&[
        "send-command",
        shell_id,
        "--command",
        command,
        "--wait-ms",
        &wait_ms.to_string(),
    ])
}
pub(crate) fn send_test_command(shell_id: &str) -> Output {
    send_command(shell_id, "Write-Output 'MCP_PTY_TEST'", 5000)
}
fn output_to_files(mut command: Command, timeout: Duration) -> Output {
    let (stdout_path, stderr_path) = output_paths();
    command
        .stdout(Stdio::from(fs::File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr_path).unwrap()));
    let status = wait_for_status(&mut command.spawn().unwrap(), timeout);
    let output = output_from_parts(
        status,
        fs::read(&stdout_path).unwrap(),
        fs::read(&stderr_path).unwrap(),
    );
    fs::remove_file(&stdout_path).unwrap();
    fs::remove_file(&stderr_path).unwrap();
    output
}
fn output_from_pipes(mut command: Command, timeout: Duration) -> Output {
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_rx = read_pipe(stdout);
    let stderr_rx = read_pipe(stderr);
    let status = wait_for_status(&mut child, timeout);
    output_from_parts(
        status,
        stdout_rx.recv_timeout(PIPE_CLOSE_TIMEOUT).unwrap(),
        stderr_rx.recv_timeout(PIPE_CLOSE_TIMEOUT).unwrap(),
    )
}
fn output_paths() -> (PathBuf, PathBuf) {
    let output_dir = std::env::temp_dir()
        .join("agent")
        .join("shell-mcp-test-output");
    fs::create_dir_all(&output_dir).unwrap();
    let prefix = format!("{}-{}", std::process::id(), uuid::Uuid::new_v4().simple());
    (
        output_dir.join(format!("{prefix}.stdout.txt")),
        output_dir.join(format!("{prefix}.stderr.txt")),
    )
}
