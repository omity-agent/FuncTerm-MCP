use super::matrix::{ShellCase, case_dir, required_executable};
use crate::support::{
    create_tab, locked_with_env, manual_write, parse_command_id, parse_command_result,
    parse_tab_view, run_cli, send_command,
};
use core::time::Duration;
use std::thread;
const PYTHON: ShellCase = ShellCase {
    name: "python",
    env_var: "FUNCTERM_PYTHON",
    executables: &["python3", "python", "python3.exe", "python.exe"],
    expected_exit_code: 0,
};
#[test]
fn cli_tools_work_with_python_repl() {
    let executable = required_executable(&PYTHON);
    let _guard = locked_with_env(&[(PYTHON.env_var, &executable)]);
    let cwd = case_dir(PYTHON.name, "repl tools");
    let created = create_tab(&cwd, PYTHON.name);
    let initial = parse_tab_view(&run_cli(&["view", &created.tab_id]));
    assert!(initial.alive, "new Python tab should be alive");
    assert!(
        !initial.screen.is_empty(),
        "new_tab and view should expose the Python screen"
    );
    assert!(
        initial.cwd.contains("repl tools"),
        "new Python tab should report its cwd: {}",
        initial.cwd
    );
    let accepted = send_command(
        &created.tab_id,
        "value = input('MCP_PTY_PYTHON_INPUT:'); print(f'MCP_PTY_PYTHON_COMMAND_{value}')",
        0.0,
    );
    let command_id = parse_command_id(&accepted);
    let pending = parse_command_result(&accepted);
    assert!(!pending.finished, "Python command should wait for input");
    let marker = "MCP_PTY_PYTHON_MANUAL_WRITE";
    let written = manual_write(
        &created.tab_id,
        format!("{marker}{}", input_terminator()).as_bytes(),
        1.0,
    );
    assert!(
        written.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&written.stdout),
        String::from_utf8_lossy(&written.stderr)
    );
    assert!(
        String::from_utf8_lossy(&written.stdout).contains(marker),
        "manual_write should return Python's updated REPL screen: stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&written.stdout),
        String::from_utf8_lossy(&written.stderr)
    );
    let completed = parse_command_result(&run_cli(&["view", &command_id, "--waiting", "10"]));
    assert!(completed.finished, "Python command should finish");
    assert_eq!(completed.exit_code, Some(0_i32));
    assert!(
        completed
            .stdout
            .contains("MCP_PTY_PYTHON_COMMAND_MCP_PTY_PYTHON_MANUAL_WRITE"),
        "stdout: {}\nstderr: {}",
        completed.stdout,
        completed.stderr
    );
}
#[test]
fn python_shim_returns_to_parent_shell_after_exit() {
    let python = required_executable(&PYTHON);
    let parent_case = parent_shell();
    let parent = required_executable(&parent_case);
    let _guard = locked_with_env(&[(PYTHON.env_var, &python), (parent_case.env_var, &parent)]);
    let created = create_tab(&case_dir(PYTHON.name, "nested shim"), parent_case.name);
    let launch = parse_command_result(&send_command(&created.tab_id, "python", 10.0));
    assert!(launch.finished, "Python shim should report ready");
    let nested = parse_command_result(&send_command(
        &created.tab_id,
        "print('MCP_PTY_NESTED_PYTHON')",
        10.0,
    ));
    assert_eq!(nested.exit_code, Some(0_i32));
    assert!(
        nested.stdout.contains("MCP_PTY_NESTED_PYTHON"),
        "stdout: {}\nstderr: {}",
        nested.stdout,
        nested.stderr
    );
    let exited = parse_command_result(&send_command(&created.tab_id, "exit()", 10.0));
    assert!(
        exited.finished,
        "Python exit should finish before leaving REPL"
    );
    thread::sleep(Duration::from_millis(500));
    let restored = parse_command_result(&send_command(
        &created.tab_id,
        parent_marker_command(),
        10.0,
    ));
    assert_eq!(restored.exit_code, Some(0_i32));
    assert!(restored.stdout.contains("MCP_PTY_PARENT_SHELL"));
}
#[cfg(windows)]
const fn input_terminator() -> &'static str {
    "\r"
}
#[cfg(not(windows))]
const fn input_terminator() -> &'static str {
    "\n"
}
#[cfg(windows)]
const fn parent_shell() -> ShellCase {
    ShellCase {
        name: "cmd",
        env_var: "FUNCTERM_CMD",
        executables: &["cmd.exe"],
        expected_exit_code: 7,
    }
}
#[cfg(not(windows))]
const fn parent_shell() -> ShellCase {
    ShellCase {
        name: "bash",
        env_var: "FUNCTERM_BASH",
        executables: &["bash", "bash.exe"],
        expected_exit_code: 1,
    }
}
#[cfg(windows)]
const fn parent_marker_command() -> &'static str {
    "echo MCP_PTY_PARENT_SHELL"
}
#[cfg(not(windows))]
const fn parent_marker_command() -> &'static str {
    "printf 'MCP_PTY_PARENT_SHELL\\n'"
}
