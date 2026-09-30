#[path = "input_flow/fixtures.rs"]
mod fixtures;
#[cfg(windows)]
#[path = "input_flow/handoff.rs"]
mod handoff;
#[cfg(test)]
#[path = "input_flow/redirection.rs"]
mod redirection;
use crate::support::{create_tab, parse_command_id, parse_command_result, run_cli, send_command};
use fixtures::{assert_parent, pipe_command, start_runtime};
#[test]
fn terminal_python_still_enters_a_managed_session() {
    let (_guard, cwd, parent) = start_runtime("terminal repl");
    let created = create_tab(&cwd, parent);
    let launch = parse_command_result(&send_command(&created.tab_id, "python", 10.0));
    assert!(launch.finished);
    assert_eq!(launch.exit_code, Some(0_i32));
    let result = parse_command_result(&send_command(
        &created.tab_id,
        "print('MANAGED_REPL_READY')",
        10.0,
    ));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32));
    assert!(
        result.stdout.contains("MANAGED_REPL_READY"),
        "stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
}
#[test]
fn piped_python_preserves_the_script_and_parent_command_sequence() {
    let (_guard, cwd, parent) = start_runtime("pipeline sequence");
    let created = create_tab(&cwd, parent);
    let script = "for name in ('yaml', 'ruamel', 'pytest', 'ruff', 'zuban'):\n    print(name)\n";
    let command = format!(
        "{}\n{}",
        pipe_command(parent, script, "python"),
        fixtures::parent_marker(parent)
    );
    let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32));
    for marker in [
        "yaml",
        "ruamel",
        "pytest",
        "ruff",
        "zuban",
        "PARENT_CONTINUED",
    ] {
        assert!(result.stdout.contains(marker), "stdout: {}", result.stdout);
    }
    assert!(result.stderr.is_empty(), "stderr: {}", result.stderr);
    assert_parent(&created.tab_id, parent);
}
#[test]
fn redirected_python_flags_and_aliases_preserve_streams_and_exit_status() {
    let (_guard, cwd, parent) = start_runtime("pipeline arguments");
    let created = create_tab(&cwd, parent);
    let script =
        "import sys\nprint('PIPE_STDOUT')\nprint('PIPE_STDERR', file=sys.stderr)\nsys.exit(7)\n";
    for target in [
        "python",
        "python -u",
        "python -q",
        "python -i",
        "python3",
        "pypy3",
    ] {
        let command = pipe_command(parent, script, target);
        let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
        assert!(result.finished, "{target} did not finish");
        assert_eq!(result.exit_code, Some(7_i32), "{target}: {}", result.stderr);
        assert!(
            result.stdout.contains("PIPE_STDOUT"),
            "{target}: {}",
            result.stdout
        );
        assert!(
            result.stderr.contains("PIPE_STDERR"),
            "{target}: {}",
            result.stderr
        );
        assert!(!result.stdout.contains("PIPE_STDERR"));
        assert!(!result.stderr.contains("PIPE_STDOUT"));
        assert_parent(&created.tab_id, parent);
    }
}
#[test]
fn piped_process_stays_pending_until_native_execution_finishes() {
    let (_guard, cwd, parent) = start_runtime("pipeline completion");
    let created = create_tab(&cwd, parent);
    let script = "import time\ntime.sleep(1)\nprint('NATIVE_FINISHED')\n";
    let accepted = send_command(
        &created.tab_id,
        &pipe_command(parent, script, "python"),
        0.0,
    );
    assert!(!parse_command_result(&accepted).finished);
    let command_id = parse_command_id(&accepted);
    let result = parse_command_result(&run_cli(&["view", &command_id, "--wait-timeout", "10"]));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32));
    assert!(
        result.stdout.contains("NATIVE_FINISHED"),
        "stdout: {}",
        result.stdout
    );
    assert_parent(&created.tab_id, parent);
}
#[test]
fn piped_bash_does_not_take_over_the_terminal() {
    let (_guard, cwd, parent) = start_runtime("pipeline shell");
    let created = create_tab(&cwd, parent);
    for target in ["bash", "bash -i"] {
        let command = pipe_command(parent, "printf '%s\\n' 'BASH_PIPE_RAN'\nexit 7\n", target);
        let result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
        assert!(result.finished, "{target} did not finish");
        assert_eq!(result.exit_code, Some(7_i32), "stderr: {}", result.stderr);
        assert!(
            result.stdout.contains("BASH_PIPE_RAN"),
            "stdout: {}",
            result.stdout
        );
        assert_parent(&created.tab_id, parent);
    }
}
