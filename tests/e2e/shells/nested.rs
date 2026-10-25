use super::matrix::{
    case_dir, exit_command, nested_launch_command, nested_marker_command, shell_cases,
};
use crate::support::{
    command_directory, create_tab, parse_command_id, parse_command_result, run_cli, send_command,
};
use core::time::Duration;
use std::time::Instant;
#[test]
fn cli_keeps_nested_launch_result_stable_after_nested_shell_exits() {
    let _guard = super::matrix::daemon();
    for case in shell_cases() {
        let tab = create_tab(&case_dir(case.name, "nested start"), case.name);
        let output = send_command(&tab.tab_id, nested_launch_command(case.name), 15.0);
        let id = parse_command_id(&output);
        let launch = parse_command_result(&output);
        assert!(launch.finished, "{}: {launch:?}", case.name);
        assert_eq!(launch.exit_code, Some(0_i32), "{}: {launch:?}", case.name);
        let directory = command_directory(&tab.tab_id, &id);
        let source = if case.name == "cmd" {
            "command.cmd"
        } else {
            "command.txt"
        };
        assert!(
            directory.join(source).is_file(),
            "live wrapper lost input: {}",
            case.name
        );
        let marker = format!("MCP_PTY_NESTED_{}", case.name.to_ascii_uppercase());
        let nested = parse_command_result(&send_command(
            &tab.tab_id,
            &nested_marker_command(case.name, &marker),
            5.0,
        ));
        assert!(nested.stdout.contains(&marker), "{nested:?}");
        let exited = parse_command_result(&send_command(&tab.tab_id, exit_command(case.name), 5.0));
        assert!(exited.finished, "{}: {exited:?}", case.name);
        if case.name == "cmd" {
            assert_eq!(exited.exit_code, Some(42_i32));
        }
        let saved = parse_command_result(&run_cli(&["view", &id]));
        assert_eq!(saved.exit_code, Some(0_i32), "{saved:?}");
        let restored = parse_command_result(&send_command(
            &tab.tab_id,
            &nested_marker_command(case.name, "PARENT_RESTORED"),
            5.0,
        ));
        assert_eq!(restored.exit_code, Some(0_i32), "{restored:?}");
        assert!(restored.stdout.contains("PARENT_RESTORED"), "{restored:?}");
        let started = Instant::now();
        while directory.try_exists().unwrap() && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !directory.try_exists().unwrap(),
            "launch record not released: {}",
            directory.display()
        );
    }
}
