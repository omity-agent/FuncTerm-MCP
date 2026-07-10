use super::matrix::{
    case_dir, exit_command, nested_launch_command, nested_marker_command, required_executable,
    shell_cases,
};
use crate::support::{
    create_tab, locked_with_env, parse_command_id, parse_command_result, run_cli, send_command,
};
use core::time::Duration;
use std::thread;
#[cfg(windows)]
#[test]
fn cli_launches_nested_nushell_through_shim_after_implicit_capture_change() {
    let case = shell_cases().iter().find(|case| case.name == "nu").unwrap();
    let executable = required_executable(case);
    let _guard = locked_with_env(&[(case.env_var, &executable)]);
    let created = create_tab(&case_dir(case.name, "nested shim"), case.name);
    let launch_output = send_command(&created.tab_id, nested_launch_command(case.name), 10.0);
    let launch_id = parse_command_id(&launch_output);
    let launch = parse_command_result(&launch_output);
    assert!(launch.finished, "nested nu launch should finish");
    assert_eq!(launch.exit_code, Some(0_i32));
    let marker = "MCP_PTY_NESTED_NU_SHIM";
    let nested = parse_command_result(&send_command(
        &created.tab_id,
        &nested_marker_command(case.name, marker),
        10.0,
    ));
    assert!(
        nested.stdout.contains(marker),
        "nested nu stdout should include marker: {}",
        nested.stdout
    );
    let _closed = send_command(&created.tab_id, exit_command(case.name), 0.2);
    thread::sleep(Duration::from_millis(500));
    let launch_after_exit = parse_command_result(&run_cli(&["view", &launch_id]));
    assert_eq!(launch_after_exit.exit_code, Some(0_i32));
}
#[test]
fn cli_keeps_nested_launch_result_stable_after_nested_shell_exits() {
    for case in shell_cases() {
        let executable = required_executable(case);
        let _guard = locked_with_env(&[(case.env_var, &executable)]);
        let created = create_tab(&case_dir(case.name, "nested start"), case.name);
        let launch_output = send_command(&created.tab_id, nested_launch_command(case.name), 10.0);
        let launch_id = parse_command_id(&launch_output);
        let launch = parse_command_result(&launch_output);
        assert!(launch.finished, "{} nested launch should finish", case.name);
        assert_eq!(launch.exit_code, Some(0_i32));
        let marker = format!("MCP_PTY_NESTED_{}", case.name.to_ascii_uppercase());
        let nested = parse_command_result(&send_command(
            &created.tab_id,
            &nested_marker_command(case.name, &marker),
            10.0,
        ));
        assert!(
            nested.stdout.contains(&marker),
            "{name} nested stdout should include marker: {stdout}",
            name = case.name,
            stdout = nested.stdout
        );
        let _closed = send_command(&created.tab_id, exit_command(case.name), 0.2);
        thread::sleep(Duration::from_millis(500));
        let launch_after_exit = parse_command_result(&run_cli(&["view", &launch_id]));
        assert_eq!(launch_after_exit.exit_code, Some(0_i32));
    }
}
