use crate::support::{
    create_tab, locked_with_env, parse_command_id, parse_command_result, parse_tab_view, run_cli,
    send_command, temp_root,
};
use core::time::Duration;
use std::time::Instant;
#[cfg(windows)]
const SHELL: &str = "powershell";
#[cfg(unix)]
const SHELL: &str = "bash";
#[test]
fn close_idle_tab_preserves_history_and_other_tabs() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let accepted = send_command(&target.tab_id, "echo CLOSE_HISTORY", 5.0);
    let command_id = parse_command_id(&accepted);
    assert!(parse_command_result(&accepted).finished);
    let closed = run_cli(&["close", "--tab-id", &target.tab_id]);
    assert!(
        closed.status.success(),
        "{}",
        String::from_utf8_lossy(&closed.stderr)
    );
    assert!(String::from_utf8_lossy(&closed.stdout).contains(&target.tab_id));
    assert!(!parse_tab_view(&run_cli(&["view", &target.tab_id])).alive);
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
    let history = parse_command_result(&run_cli(&["view", &command_id]));
    assert!(history.finished);
    assert!(history.stdout.contains("CLOSE_HISTORY"));
    assert!(
        !send_command(&target.tab_id, "echo unreachable", 0.0)
            .status
            .success()
    );
    assert!(
        run_cli(&["close", "--tab-id", &target.tab_id])
            .status
            .success()
    );
}
#[test]
fn close_current_from_nested_shell_targets_its_own_tab() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let accepted = send_command(&target.tab_id, SHELL, 0.0);
    let command_id = parse_command_id(&accepted);
    let nested = run_cli(&["view", &command_id, "--wait-timeout", "10"]);
    assert!(
        parse_command_result(&nested).finished,
        "nested launch: {nested:?}; terminal: {:?}",
        run_cli(&["view", &target.tab_id])
    );
    #[cfg(windows)]
    let command = "& $env:FUNCTERM_HELPER_EXECUTABLE close --current";
    #[cfg(unix)]
    let command = "\"$FUNCTERM_HELPER_EXECUTABLE\" close --current";
    let closed = send_command(&target.tab_id, command, 5.0);
    assert!(parse_command_result(&closed).finished);
    assert!(
        !parse_tab_view(&run_cli(&["view", &target.tab_id])).alive,
        "close --current response: {}",
        String::from_utf8_lossy(&closed.stdout)
    );
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
}
#[test]
fn shell_exit_without_done_file_settles_the_running_command() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    #[cfg(windows)]
    let command = "Start-Sleep -Seconds 1; Stop-Process -Id $PID -Force";
    #[cfg(unix)]
    let command = "sleep 1; kill -KILL $$";
    let accepted = send_command(&target.tab_id, command, 0.0);
    let command_id = parse_command_id(&accepted);
    let started = Instant::now();
    let output = run_cli(&["view", &command_id, "--wait-timeout", "5"]);
    let result = parse_command_result(&output);
    assert!(
        result.finished,
        "shell termination did not settle command: {output:?}"
    );
    assert_ne!(result.exit_code, Some(0_i32));
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "shell exit was only noticed after the client wait timed out"
    );
    let cached = parse_command_result(&run_cli(&["view", &command_id]));
    assert_eq!(cached.exit_code, result.exit_code);
    assert!(!parse_tab_view(&run_cli(&["view", &target.tab_id])).alive);
}
