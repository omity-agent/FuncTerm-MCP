use super::daemon;
use super::parse::{
    CommandResult, TabCreated, parse_command_id, parse_command_result, parse_tab_created,
};
use super::process::read_pipe;
use super::process::wait_for_status;
use base64_turbo::STANDARD;
use core::cell::RefCell;
use core::time::Duration;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;
pub(crate) const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(20);
const PIPE_CLOSE_TIMEOUT: Duration = Duration::from_secs(5);
thread_local! {
    static TAB_SEPARATE_OUTPUT: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
}
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
    let created = parse_tab_created(&run_cli_with_env(
        &[
            "new-tab",
            "--no-profile",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            shell,
        ],
        env,
    ));
    TAB_SEPARATE_OUTPUT.with(|tabs| {
        tabs.borrow_mut()
            .insert(created.tab_id.clone(), shell != "nu");
    });
    created
}
pub(crate) fn send_command(tab_id: &str, command: &str, wait_timeout: f64) -> Output {
    let separate_output = TAB_SEPARATE_OUTPUT
        .with(|tabs| tabs.borrow().get(tab_id).copied().unwrap_or(true))
        .to_string();
    run_cli(&[
        "send-command",
        tab_id,
        "--command",
        command,
        "--separate-output",
        &separate_output,
        "--wait-timeout",
        &wait_timeout.to_string(),
    ])
}
pub(crate) fn complete_command(tab_id: &str, command: &str) -> CommandResult {
    wait_for_completion(&send_command(tab_id, command, 0.0))
}
pub(crate) fn wait_for_completion(accepted: &Output) -> CommandResult {
    let id = parse_command_id(accepted);
    let deadline = Instant::now() + CLI_COMMAND_TIMEOUT;
    loop {
        let observed = parse_command_result(&run_cli(&["view", &id, "--wait-timeout", "1"]));
        if observed.finished {
            return observed;
        }
        assert!(
            Instant::now() < deadline,
            "command {id} did not finish within {CLI_COMMAND_TIMEOUT:?}: {observed:?}"
        );
    }
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
