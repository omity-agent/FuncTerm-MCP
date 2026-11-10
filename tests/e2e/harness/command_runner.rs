use super::daemon;
use super::parse::{
    CommandResult, TabCreated, parse_command_id, parse_command_result, parse_tab_created,
};
use base64_turbo::STANDARD;
use core::cell::RefCell;
use core::time::Duration;
use process_control::{ChildExt as _, Control as _};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;
pub(crate) const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
thread_local! {
    static TAB_SEPARATE_OUTPUT: RefCell<HashMap<String, bool>> = RefCell::new(HashMap::new());
}
pub(crate) fn exe() -> PathBuf {
    std::env::var_os("FUNCTERM_TEST_EXECUTABLE").map_or_else(
        || PathBuf::from(env!("CARGO_BIN_EXE_functerm")),
        PathBuf::from,
    )
}
pub(crate) fn run_cli(arguments: &[&str]) -> Output {
    let mut command = Command::new(exe());
    command
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    daemon::apply_active_env(&mut command);
    command
        .spawn()
        .unwrap()
        .controlled_with_output()
        .time_limit(CLI_COMMAND_TIMEOUT)
        .terminate_for_timeout()
        .strict_errors()
        .wait()
        .unwrap()
        .unwrap_or_else(|| {
            panic!("CLI command {command:?} timed out after {CLI_COMMAND_TIMEOUT:?}")
        })
        .into_std_lossy()
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
    let separate_output =
        TAB_SEPARATE_OUTPUT.with(|tabs| tabs.borrow().get(tab_id).copied().unwrap_or(true));
    send_command_with_output(tab_id, command, separate_output, wait_timeout)
}
pub(crate) fn send_command_with_output(
    tab_id: &str,
    command: &str,
    separate_output: bool,
    wait_timeout: f64,
) -> Output {
    run_cli(&[
        "send-command",
        tab_id,
        "--command",
        command,
        "--separate-output",
        &separate_output.to_string(),
        "--wait-timeout",
        &wait_timeout.to_string(),
    ])
}
pub(crate) fn wait_for_stdout(
    command_id: &str,
    expected: &str,
    timeout: Duration,
) -> CommandResult {
    let started = Instant::now();
    loop {
        let result = parse_command_result(&run_cli(&["view", command_id, "--wait-timeout", "0.1"]));
        assert!(
            !result.finished,
            "command ended before producing {expected}: {result:?}"
        );
        if result.stdout.contains(expected) {
            return result;
        }
        assert!(
            started.elapsed() < timeout,
            "command did not produce {expected}: {result:?}"
        );
    }
}
pub(crate) fn complete_command(tab_id: &str, command: &str) -> CommandResult {
    wait_for_completion(&send_command(tab_id, command, 0.0))
}
pub(crate) fn wait_for_completion(accepted: &Output) -> CommandResult {
    let id = parse_command_id(accepted);
    let initial = parse_command_result(accepted);
    if initial.finished {
        return initial;
    }
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
