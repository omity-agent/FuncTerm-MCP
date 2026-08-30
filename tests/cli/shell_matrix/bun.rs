use super::matrix::{ShellCase, case_dir, required_executable};
use crate::support::{
    create_tab, locked_with_env, manual_write, parse_command_id, parse_command_result,
    parse_tab_view, run_cli, send_command,
};
use core::time::Duration;
use std::thread;
const BUN: ShellCase = ShellCase {
    name: "bun",
    env_var: "FUNCTERM_BUN",
    executables: &["bun", "bun.exe"],
    expected_exit_code: 0,
};
#[test]
fn cli_tools_work_with_bun_repl() {
    let executable = required_executable(&BUN);
    let _guard = locked_with_env(&[(BUN.env_var, &executable)]);
    let cwd = case_dir(BUN.name, "repl tools");
    let created = create_tab(&cwd, BUN.name);
    let initial = parse_tab_view(&run_cli(&["view", &created.tab_id]));
    assert!(initial.alive, "new Bun tab should be alive");
    let accepted = send_command(
        &created.tab_id,
        "await new Promise(resolve => process.stdin.once('data', data => { console.log(`MCP_PTY_BUN_COMMAND_${data.toString().trim()}`); resolve(); }))",
        0.0,
    );
    let command_id = parse_command_id(&accepted);
    let pending = parse_command_result(&accepted);
    assert!(!pending.finished, "Bun command should still be running");
    let marker = "MCP_PTY_BUN_MANUAL_WRITE";
    let written = manual_write(&created.tab_id, format!("{marker}\r").as_bytes(), 1.0);
    assert!(
        written.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&written.stdout),
        String::from_utf8_lossy(&written.stderr)
    );
    assert!(
        String::from_utf8_lossy(&written.stdout).contains(marker),
        "manual_write should return Bun's updated REPL screen: stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&written.stdout),
        String::from_utf8_lossy(&written.stderr)
    );
    let completed = parse_command_result(&run_cli(&["view", &command_id, "--waiting", "10"]));
    assert!(completed.finished, "Bun command should finish");
    assert_eq!(completed.exit_code, Some(0_i32));
    assert!(
        completed
            .stdout
            .contains("MCP_PTY_BUN_COMMAND_MCP_PTY_BUN_MANUAL_WRITE")
    );
}
#[test]
fn bun_shim_returns_to_parent_shell_after_exit() {
    let bun = required_executable(&BUN);
    let parent_case = parent_shell();
    let parent = required_executable(&parent_case);
    let _guard = locked_with_env(&[(BUN.env_var, &bun), (parent_case.env_var, &parent)]);
    let created = create_tab(&case_dir(BUN.name, "nested shim"), parent_case.name);
    let launch = parse_command_result(&send_command(&created.tab_id, "bun", 10.0));
    assert!(launch.finished, "Bun shim should report ready");
    let nested = parse_command_result(&send_command(
        &created.tab_id,
        "console.log('MCP_PTY_NESTED_BUN')",
        10.0,
    ));
    assert!(
        nested.stdout.contains("MCP_PTY_NESTED_BUN"),
        "stdout: {}\nstderr: {}",
        nested.stdout,
        nested.stderr
    );
    let exited = parse_command_result(&send_command(&created.tab_id, ".exit", 10.0));
    assert!(
        exited.finished,
        "Bun .exit should finish before leaving the REPL"
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
