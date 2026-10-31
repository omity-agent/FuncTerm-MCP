use crate::support::{
    create_tab, locked_with_env, parse_command_id, parse_command_result, parse_tab_view, run_cli,
    send_command, temp_root, wait_for_completion,
};
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
    assert!(wait_for_completion(&accepted).finished);
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
    let nested = wait_for_completion(&accepted);
    assert!(
        nested.finished,
        "nested launch: {nested:?}; terminal: {:?}",
        run_cli(&["view", &target.tab_id])
    );
    #[cfg(windows)]
    let command = "& $env:FUNCTERM_HELPER_EXECUTABLE close --current";
    #[cfg(unix)]
    let command = "\"$FUNCTERM_HELPER_EXECUTABLE\" close --current";
    let closed = send_command(&target.tab_id, command, 5.0);
    assert!(wait_for_completion(&closed).finished);
    assert!(
        !parse_tab_view(&run_cli(&["view", &target.tab_id])).alive,
        "close --current response: {}",
        String::from_utf8_lossy(&closed.stdout)
    );
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
}
