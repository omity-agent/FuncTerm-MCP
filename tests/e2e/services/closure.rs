use crate::support::{
    command_directory, create_tab, locked_with_env, parse_command_id, parse_command_result,
    parse_tab_view, run_cli, run_cli_with_env, send_command, temp_root, wait_for_completion,
};
#[cfg(windows)]
const SHELL: &str = "powershell";
#[cfg(unix)]
const SHELL: &str = "bash";
#[test]
fn close_and_reader_failure_preserve_history_and_other_tabs() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let accepted = send_command(&target.tab_id, "echo CLOSE_HISTORY", 5.0);
    let command_id = parse_command_id(&accepted);
    wait_for_completion(&accepted);
    let workspace = command_directory(&target.tab_id, &command_id)
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf();
    let closed = run_cli(&["close", "--tab-id", &target.tab_id]);
    assert!(closed.status.success(), "{closed:?}");
    assert!(String::from_utf8_lossy(&closed.stdout).contains(&target.tab_id));
    let deadline = std::time::Instant::now() + core::time::Duration::from_secs(5);
    while workspace.try_exists().unwrap() {
        assert!(
            std::time::Instant::now() < deadline,
            "closed Tab workspace was retained"
        );
        std::thread::sleep(core::time::Duration::from_millis(10));
    }
    assert!(!parse_tab_view(&run_cli(&["view", &target.tab_id])).alive);
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
    let history = parse_command_result(&run_cli(&["view", &command_id]));
    assert!(history.finished);
    assert!(history.stdout.contains("CLOSE_HISTORY"));
    let rejected = send_command(&target.tab_id, "echo unreachable", 0.0);
    assert!(!rejected.status.success(), "{rejected:?}");
    let repeated = run_cli(&["close", "--tab-id", &target.tab_id]);
    assert!(repeated.status.success(), "{repeated:?}");
    let failing = create_tab(&temp_root(), "bun").tab_id;
    let submitted = send_command(
        &failing,
        "console.log('BEFORE_READER_FAILURE\\x1b]9999;FuncTerm;start;missing\\x1b\\\\'); await Bun.sleep(30000)",
        0.0,
    );
    let failed = wait_for_completion(&submitted);
    assert_eq!(failed.exit_code, Some(1_i32), "{failed:?}");
    assert!(
        failed.stdout.contains("BEFORE_READER_FAILURE"),
        "{failed:?}"
    );
    assert!(!parse_tab_view(&run_cli(&["view", &failing])).alive);
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
}
#[test]
fn close_current_from_nested_shell_targets_its_own_tab() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let accepted = send_command(&target.tab_id, SHELL, 0.0);
    wait_for_completion(&accepted);
    #[cfg(windows)]
    let command = "& $env:FUNCTERM_HELPER_EXECUTABLE close --current";
    #[cfg(unix)]
    let command = "\"$FUNCTERM_HELPER_EXECUTABLE\" close --current";
    let closed = send_command(&target.tab_id, command, 5.0);
    wait_for_completion(&closed);
    assert!(
        !parse_tab_view(&run_cli(&["view", &target.tab_id])).alive,
        "close --current response: {}",
        String::from_utf8_lossy(&closed.stdout)
    );
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
}
#[test]
fn failed_startup_removes_its_runtime_directory() {
    let guard = locked_with_env(&[("FUNCTERM_SHELL_LOAD_PROFILE", "true")]);
    let root = temp_root();
    let tab = create_tab(&root, "python");
    let workspace = command_directory(&tab.tab_id, "unused");
    let tabs = workspace.ancestors().nth(3).unwrap();
    let before = fs_err::read_dir(tabs).unwrap().count();
    let startup = root.join("exit-startup.py");
    fs_err::write(&startup, "raise SystemExit(23)\n").unwrap();
    let mut environment = guard.env();
    environment.push((
        "PYTHONSTARTUP".to_owned(),
        startup.to_str().unwrap().to_owned(),
    ));
    let failed = run_cli_with_env(
        &[
            "new-tab",
            "--starting-shell",
            "python",
            "--starting-directory",
            root.to_str().unwrap(),
        ],
        &environment,
    );
    assert!(!failed.status.success(), "{failed:?}");
    assert_eq!(fs_err::read_dir(tabs).unwrap().count(), before);
    assert!(parse_tab_view(&run_cli(&["view", &tab.tab_id])).alive);
}
