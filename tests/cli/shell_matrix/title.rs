use super::matrix::{
    case_dir, plain_title_command, required_executable, set_title_command, shell_cases,
};
use crate::support::{
    create_tab, locked_with_env, parse_command_id, parse_command_result, parse_tab_view, run_cli,
    send_command,
};
const MODEL_TITLE: &str = "FuncTerm";
#[cfg(windows)]
#[test]
fn cli_distinguishes_native_host_title_from_command_title() {
    let case = shell_cases()
        .iter()
        .find(|case| case.name == "powershell")
        .unwrap();
    let executable = required_executable(case);
    let _guard = locked_with_env(&[(case.env_var, &executable)]);
    let created = create_tab(&case_dir(case.name, "native command title"), case.name);
    let result = parse_command_result(&send_command(&created.tab_id, "where.exe where.exe", 10.0));
    assert_finished(&result, case.name, "where.exe");
    let tab = parse_tab_view(&run_cli(&["view", &created.tab_id]));
    assert_eq!(
        (result.title.as_str(), tab.title.as_str()),
        (MODEL_TITLE, MODEL_TITLE),
        "neither a command nor its tab may expose PowerShell's host window title"
    );
    let intended_title = "MCP_PTY_INTENTIONAL_TITLE";
    let title_then_native = format!(
        "{}; where.exe where.exe",
        set_title_command(case.name, intended_title)
    );
    let titled = parse_command_result(&send_command(&created.tab_id, &title_then_native, 10.0));
    assert_finished(&titled, case.name, "MCP_PTY_TITLE_SET");
    assert_eq!(
        titled.title, intended_title,
        "an intentional command title must remain observable"
    );
}
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
            first.title, MODEL_TITLE,
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
        let tab = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_eq!(
            tab.title, MODEL_TITLE,
            "{} tab title must remain the model title",
            case.name
        );
        let historical = parse_command_result(&run_cli(&["view", &first_command_id]));
        assert_eq!(
            historical.title, MODEL_TITLE,
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
            last.title, MODEL_TITLE,
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
