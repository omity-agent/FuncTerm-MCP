use super::matrix::{
    case_dir, exit_command, nested_launch_command, nested_marker_command, required_executable,
    shell_cases,
};
use crate::support::{
    command_directory, create_tab, locked_with_env, parse_command_id, parse_command_result,
    run_cli, send_command,
};
use core::time::Duration;
use std::time::Instant;
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
        assert_eq!(
            launch.exit_code,
            Some(0_i32),
            "{} nested launch failed: stdout: {}\nstderr: {}",
            case.name,
            launch.stdout,
            launch.stderr
        );
        let launch_directory = command_directory(&created.tab_id, &launch_id);
        assert!(
            launch_directory.join("input").is_dir(),
            "{} removed launch inputs while its wrapper was still running",
            case.name
        );
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
        let exit_output = send_command(&created.tab_id, exit_command(case.name), 10.0);
        let exited = parse_command_result(&exit_output);
        assert!(
            exited.finished,
            "{} nested exit did not finish: {}\nterminal snapshot: {:?}",
            case.name,
            String::from_utf8_lossy(&exit_output.stdout),
            run_cli(&["view", &created.tab_id])
        );
        if case.name == "cmd" {
            assert_eq!(exited.exit_code, Some(42_i32));
        }
        let launch_after_exit = parse_command_result(&run_cli(&["view", &launch_id]));
        assert_eq!(launch_after_exit.exit_code, Some(0_i32));
        let restored = parse_command_result(&send_command(
            &created.tab_id,
            &nested_marker_command(case.name, "PARENT_RESTORED"),
            10.0,
        ));
        assert_eq!(
            restored.exit_code,
            Some(0_i32),
            "{} parent command failed after nested exit: stdout: {}\nstderr: {}",
            case.name,
            restored.stdout,
            restored.stderr
        );
        assert!(restored.stdout.contains("PARENT_RESTORED"));
        let cleanup_started = Instant::now();
        while launch_directory.try_exists().unwrap()
            && cleanup_started.elapsed() < Duration::from_secs(3)
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !launch_directory.try_exists().unwrap(),
            "{} did not release its completed launch record: released={} early_guard={}",
            case.name,
            launch_directory
                .join("state")
                .join("released")
                .try_exists()
                .unwrap(),
            launch_directory
                .parent()
                .unwrap()
                .join(".early-done")
                .join(&launch_id)
                .try_exists()
                .unwrap()
        );
    }
}
