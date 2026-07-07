#[cfg(test)]
#[path = "shell_matrix/matrix_support.rs"]
mod matrix;
#[cfg(test)]
mod tests {
    use super::matrix::{
        assert_cwd, assert_shell_query, case_command, case_dir, exit_command,
        immediately_exiting_executable, nested_launch_command, nested_marker_command,
        required_executable, shell_cases,
    };
    use crate::support::{
        create_tab, locked_with_env, parse_command_id, parse_command_result, parse_tab_view,
        run_cli, send_command,
    };
    use core::time::Duration;
    #[cfg(windows)]
    use std::fs;
    use std::thread;
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
            let command_result =
                parse_command_result(&send_command(&created.tab_id, &command, 10.0));
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
            assert_cwd(&command_result.cwd, &next, case.name);
            let shell_after = parse_tab_view(&run_cli(&["view", &created.tab_id]));
            assert_shell_query(&shell_after, &next, case.name);
        }
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
            let launch_output =
                send_command(&created.tab_id, nested_launch_command(case.name), 10.0);
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
}
