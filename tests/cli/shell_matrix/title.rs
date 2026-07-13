use super::matrix::{
    case_dir, plain_title_command, required_executable, set_title_command, shell_cases,
};
use crate::support::{
    create_tab, locked_with_env, parse_command_id, parse_command_result, run_cli, send_command,
};
const INITIAL_TITLE: &str = "FuncTerm";
#[test]
fn cli_reports_titles_for_each_command() {
    for case in shell_cases() {
        let executable = required_executable(case);
        let _guard = locked_with_env(&[(case.env_var, &executable)]);
        let created = create_tab(&case_dir(case.name, "command titles"), case.name);
        let first_output = send_command(&created.tab_id, plain_title_command(case.name), 10.0);
        let first_command_id = parse_command_id(&first_output);
        let first = parse_command_result(&first_output);
        assert_finished(&first, case.name, "MCP_PTY_PLAIN_TITLE");
        assert_eq!(
            first.title, INITIAL_TITLE,
            "{} command without a title should use the initial title",
            case.name
        );
        let expected_title = format!("MCP_PTY_{}_COMMAND_TITLE", case.name.to_ascii_uppercase());
        let titled = parse_command_result(&send_command(
            &created.tab_id,
            &set_title_command(case.name, &expected_title),
            10.0,
        ));
        assert_finished(&titled, case.name, "MCP_PTY_TITLE_SET");
        assert_command_title(&titled.title, &expected_title, case.name);
        let historical = parse_command_result(&run_cli(&["view", &first_command_id]));
        assert_eq!(
            historical.title, INITIAL_TITLE,
            "{} historical command title should not follow the live shell title",
            case.name
        );
        let last = parse_command_result(&send_command(
            &created.tab_id,
            plain_title_command(case.name),
            10.0,
        ));
        assert_finished(&last, case.name, "MCP_PTY_PLAIN_TITLE");
        assert_eq!(
            last.title, INITIAL_TITLE,
            "{} later command without a title should not inherit a prior command title",
            case.name
        );
    }
}
fn assert_finished(result: &crate::support::CommandResult, shell: &str, output_marker: &str) {
    assert!(result.finished, "{shell} title command should finish");
    assert_eq!(
        result.exit_code,
        Some(0_i32),
        "{shell} title command failed: stdout: {}\\nstderr: {}",
        result.stdout,
        result.stderr
    );
    assert!(
        result.stdout.contains(output_marker),
        "{shell} title command stdout should include {output_marker}: {}",
        result.stdout
    );
}
fn assert_command_title(actual: &str, expected: &str, shell: &str) {
    if shell == "cmd" {
        assert!(
            actual.contains(expected),
            "CMD may decorate its native title, but should preserve {expected}: {actual}"
        );
    } else {
        assert_eq!(
            actual, expected,
            "{shell} should report the title set by the command"
        );
    }
}
