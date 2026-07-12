use super::daemon;
use super::parse::{TabCreated, parse_tab_created};
#[cfg(windows)]
use super::process::read_pipe;
use super::process::{output_from_parts, wait_for_status};
use super::temp;
use base64_turbo::STANDARD;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
pub(crate) const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
#[cfg(windows)]
const PIPE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
static OUTPUT_COUNTER: AtomicU64 = AtomicU64::new(0);
pub(crate) fn exe() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_functerm"))
}
pub(crate) fn run_cli(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command.args(arguments).stdin(Stdio::null());
    daemon::apply_active_env(&mut command);
    output_to_files(command, CLI_COMMAND_TIMEOUT)
}
pub(crate) fn run_cli_with_env(arguments: &[&str], env: &[(String, String)]) -> Output {
    let previous = daemon::active_env();
    daemon::set_active_env(env);
    let output = run_cli(arguments);
    daemon::set_active_env(&previous);
    output
}
#[cfg(windows)]
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
pub(crate) fn create_tab(cwd: &Path, shell: &str) -> TabCreated {
    create_tab_from_directory_argument(cwd.to_str().unwrap(), shell)
}
pub(crate) fn create_tab_with_env(cwd: &Path, shell: &str, env: &[(String, String)]) -> TabCreated {
    parse_tab_created(&run_cli_with_env(
        &[
            "new-tab",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            shell,
        ],
        env,
    ))
}
pub(crate) fn create_tab_from_directory_argument(cwd: &str, shell: &str) -> TabCreated {
    parse_tab_created(&run_cli(&[
        "new-tab",
        "--starting-directory",
        cwd,
        "--starting-shell",
        shell,
    ]))
}
pub(crate) fn send_command(tab_id: &str, command: &str, waiting: f64) -> Output {
    run_cli(&[
        "send-command",
        tab_id,
        "--command",
        command,
        "--waiting",
        &waiting.to_string(),
    ])
}
pub(crate) fn send_command_with_env(
    env: &[(String, String)],
    tab_id: &str,
    command: &str,
    waiting: f64,
) -> Output {
    run_cli_with_env(
        &[
            "send-command",
            tab_id,
            "--command",
            command,
            "--waiting",
            &waiting.to_string(),
        ],
        env,
    )
}
pub(crate) fn manual_write(tab_id: &str, bytes: &[u8]) -> Output {
    let encoded = STANDARD.encode(bytes);
    run_cli(&["manual-write", tab_id, "--base64", &encoded])
}
fn output_to_files(mut command: Command, timeout: Duration) -> Output {
    let (stdout_path, stderr_path) = output_paths();
    command
        .stdout(Stdio::from(fs::File::create(&stdout_path).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr_path).unwrap()));
    let description = format!("{command:?}");
    let status = wait_for_status(&mut command.spawn().unwrap(), timeout, &description);
    let output = output_from_parts(
        status,
        fs::read(&stdout_path).unwrap(),
        fs::read(&stderr_path).unwrap(),
    );
    fs::remove_file(&stdout_path).unwrap();
    fs::remove_file(&stderr_path).unwrap();
    output
}
#[cfg(windows)]
fn output_from_pipes(mut command: Command, timeout: Duration) -> Output {
    let description = format!("{command:?}");
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let stdout_rx = read_pipe(stdout);
    let stderr_rx = read_pipe(stderr);
    let status = wait_for_status(&mut child, timeout, &description);
    output_from_parts(
        status,
        stdout_rx.recv_timeout(PIPE_CLOSE_TIMEOUT).unwrap(),
        stderr_rx.recv_timeout(PIPE_CLOSE_TIMEOUT).unwrap(),
    )
}
fn output_paths() -> (PathBuf, PathBuf) {
    let output_dir = temp::temp_root().join("cli-output");
    fs::create_dir_all(&output_dir).unwrap();
    let unique = OUTPUT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let prefix = format!("{}-{unique}", std::process::id());
    (
        output_dir.join(format!("{prefix}.stdout.txt")),
        output_dir.join(format!("{prefix}.stderr.txt")),
    )
}
