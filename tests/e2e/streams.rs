#[path = "routing/fixtures.rs"]
mod fixtures;
#[cfg(all(test, windows))]
#[path = "routing/handoff.rs"]
mod handoff;
#[cfg(test)]
#[path = "routing/redirection.rs"]
mod redirection;
use crate::support::{create_tab, parse_command_result, send_command};
use fixtures::{assert_parent, pipe_command, start_runtime};
#[test]
fn redirected_python_flags_and_aliases_preserve_streams_and_exit_status() {
    let (_guard, cwd, parent) = start_runtime("pipeline arguments");
    let created = create_tab(&cwd, parent);
    let script =
        "import sys\nprint('PIPE_STDOUT')\nprint('PIPE_STDERR', file=sys.stderr)\nsys.exit(7)\n";
    for target in ["python -i", "python3"] {
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
