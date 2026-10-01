use super::matrix::{
    assert_cwd, assert_shell_query, case_command, case_dir, required_executable, shell_cases,
};
use crate::support::{
    create_tab, locked_with_env, parse_command_result, parse_tab_view, run_cli, send_command,
};
#[cfg(windows)]
use std::fs;
#[test]
fn cli_runs_commands_for_every_supported_shell() {
    for case in shell_cases() {
        let executable = required_executable(case);
        let _guard = locked_with_env(&[(case.env_var, &executable)]);
        let start = case_dir(case.name, "start dir");
        let next = case_dir(case.name, "next dir");
        let created = create_tab(&start, case.name);
        let shell_before = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_before, &start, case.name);
        let command = case_command(case.name, &next);
        let command_result = parse_command_result(&send_command(&created.tab_id, &command, 10.0));
        assert!(
            command_result.finished,
            "{name} command should finish",
            name = case.name
        );
        assert!(
            command_result.stdout.contains("MCP_PTY_STDOUT"),
            "{name} stdout should include marker: {stdout}",
            name = case.name,
            stdout = command_result.stdout
        );
        assert!(
            command_result.stderr.contains("MCP_PTY_STDERR"),
            "{name} stderr should include marker: {stderr}",
            name = case.name,
            stderr = command_result.stderr
        );
        assert_eq!(command_result.exit_code, Some(case.expected_exit_code));
        assert_ne!(command_result.time_consumption, "0ns");
        assert_cwd(&command_result.cwd, &next, case.name);
        let shell_after = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_after, &next, case.name);
    }
}
#[cfg(windows)]
#[test]
fn cli_captures_nushell_implicit_structured_output() {
    let case = shell_cases().iter().find(|case| case.name == "nu").unwrap();
    let executable = required_executable(case);
    let _guard = locked_with_env(&[(case.env_var, &executable)]);
    let cwd = case_dir(case.name, "implicit output");
    let marker = "MCP_PTY_NUSHELL_IMPLICIT.txt";
    fs::write(cwd.join(marker), "implicit output marker").unwrap();
    let created = create_tab(&cwd, case.name);
    let result = parse_command_result(&send_command(
        &created.tab_id,
        &format!("ls | where name == {marker:?}"),
        10.0,
    ));
    assert!(result.finished, "nu implicit command should finish");
    assert_eq!(
        result.exit_code,
        Some(0_i32),
        "stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
    assert!(
        result.stdout.contains(marker),
        "nu stdout should include implicit table output: {}",
        result.stdout
    );
}
