use super::matrix::{
    assert_cwd, assert_shell_query, case_command, case_dir, immediately_exiting_executable,
    required_executable, shell_cases, title_probe_command,
};
use crate::support::{
    create_tab, locked_with_env, parse_command_result, parse_tab_view, run_cli, send_command,
};
use core::time::Duration;
#[cfg(windows)]
use std::fs;
use std::thread;
#[test]
fn cli_runs_commands_for_every_supported_shell() {
    let initial_title = configured_initial_title();
    let updated_title = format!("MCP_PTY_UPDATED_TITLE_{}", std::process::id());
    for case in shell_cases() {
        let executable = required_executable(case);
        let _guard = locked_with_env(&[(case.env_var, &executable)]);
        let start = case_dir(case.name, "start dir");
        let next = case_dir(case.name, "next dir");
        let created = create_tab(&start, case.name);
        let shell_before = parse_tab_view(&run_cli(&["view", &created.tab_id]));
        assert_shell_query(&shell_before, &start, case.name);
        assert_eq!(shell_before.title, initial_title, "{} title", case.name);
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
        let title_command = title_probe_command(case.name, &updated_title);
        let title_result =
            parse_command_result(&send_command(&created.tab_id, &title_command, 10.0));
        assert!(
            title_result.finished,
            "{} title probe should finish",
            case.name
        );
        wait_for_title_change(&created.tab_id, case.name, &initial_title);
    }
}
fn configured_initial_title() -> String {
    let settings = toml::from_str::<toml::Table>(include_str!("../../../settings.toml")).unwrap();
    settings
        .get("terminal_initial_title")
        .unwrap()
        .as_str()
        .unwrap()
        .to_owned()
}
fn wait_for_title_change(tab_id: &str, shell: &str, initial_title: &str) {
    for _attempt in 0_usize..50 {
        let view = parse_tab_view(&run_cli(&["view", tab_id]));
        if view.title != initial_title {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    panic!("{shell} should accept title changes after its first prompt");
}
#[test]
fn cli_reports_startup_failure_for_every_supported_shell() {
    for case in shell_cases() {
        let output = {
            let _guard = locked_with_env(&[(case.env_var, immediately_exiting_executable())]);
            let cwd = crate::support::temp_root();
            run_cli(&[
                "new-tab",
                "--starting-directory",
                cwd.to_str().unwrap(),
                "--starting-shell",
                case.name,
            ])
        };
        assert!(
            !output.status.success(),
            "{name} should fail startup",
            name = case.name
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("startup"),
            "{name} stderr should explain startup failure: {stderr}",
            name = case.name,
            stderr = String::from_utf8_lossy(&output.stderr)
        );
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
#[cfg(windows)]
#[test]
fn cli_keeps_nushell_shim_available_inside_implicit_capture() {
    let case = shell_cases().iter().find(|case| case.name == "nu").unwrap();
    let executable = required_executable(case);
    let _guard = locked_with_env(&[(case.env_var, &executable)]);
    let created = create_tab(&case_dir(case.name, "shim path"), case.name);
    let result = parse_command_result(&send_command(
        &created.tab_id,
        "which nu | get path.0",
        10.0,
    ));
    assert!(
        result.finished,
        "nu shim query should finish: stdout: {}\nstderr: {}",
        result.stdout, result.stderr
    );
    assert_eq!(
        result.exit_code,
        Some(0_i32),
        "stdout: {}\nstderr: {}",
        result.stdout,
        result.stderr
    );
    let stdout = result.stdout.replace('\\', "/").to_ascii_lowercase();
    assert!(
        stdout.contains("/shims/nu"),
        "nu command lookup should prefer FuncTerm shim: {}",
        result.stdout
    );
}
