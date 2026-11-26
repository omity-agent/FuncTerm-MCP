use crate::support::{
    SHELL, command_view, create_tab, locked_with_env, parse_command_id, run, run_cli,
    run_cli_with_env, tab_directory, tab_view, temp_root, wait_for_completion, wait_for_removal,
    wait_for_stdout,
};
#[test]
fn close_and_reader_failure_preserve_history_and_other_tabs() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let accepted = run(&target.tab_id, "echo CLOSE_HISTORY", 5.0);
    let command_id = parse_command_id(&accepted);
    wait_for_completion(&accepted);
    let workspace = tab_directory(&target.tab_id);
    #[cfg(windows)]
    let source = "Write-Output BEFORE_CLOSE; $null = [Console]::In.ReadLine()";
    #[cfg(unix)]
    let source = "echo BEFORE_CLOSE; read -r ignored";
    let running = run(&target.tab_id, source, 0.0);
    let running_id = parse_command_id(&running);
    wait_for_stdout(
        &running_id,
        "BEFORE_CLOSE",
        core::time::Duration::from_secs(10),
    );
    let closed = run_cli(&["close", "--tab-id", &target.tab_id]);
    assert!(closed.status.success(), "{closed:?}");
    assert!(String::from_utf8_lossy(&closed.stdout).contains(&target.tab_id));
    wait_for_removal(&workspace);
    assert!(!tab_view(&target.tab_id).alive);
    assert!(tab_view(&other.tab_id).alive);
    let history = command_view(&command_id);
    assert!(history.finished);
    assert!(history.stdout.contains("CLOSE_HISTORY"));
    let interrupted = wait_for_completion(&running);
    assert_eq!(interrupted.exit_code, Some(1_i32), "{interrupted:?}");
    assert!(
        interrupted.stdout.contains("BEFORE_CLOSE"),
        "{interrupted:?}"
    );
    let rejected = run(&target.tab_id, "echo unreachable", 0.0);
    assert!(!rejected.status.success(), "{rejected:?}");
    let repeated = run_cli(&["close", "--tab-id", &target.tab_id]);
    assert!(repeated.status.success(), "{repeated:?}");
    let failing = create_tab(&temp_root(), "bun").tab_id;
    let submitted = run(
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
    assert!(!tab_view(&failing).alive);
    assert!(tab_view(&other.tab_id).alive);
}
#[test]
fn idle_shell_exit_reclaims_workspace_and_retains_history() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_root();
    let tab = create_tab(&cwd, "python").tab_id;
    let workspace = tab_directory(&tab);
    let armed = run(
        &tab,
        "import os, threading, time
def close_on_signal():
    while not os.path.exists('exit-now'):
        time.sleep(0.02)
    os._exit(23)
threading.Thread(target=close_on_signal, daemon=True).start()
print('EXIT_ARMED')",
        0.0,
    );
    let command_id = parse_command_id(&armed);
    let result = wait_for_completion(&armed);
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    fs_err::write(cwd.join("exit-now"), "").unwrap();
    wait_for_removal(&workspace);
    let closed = tab_view(&tab);
    assert!(!closed.alive, "{}", closed.screen);
    let history = command_view(&command_id);
    assert_eq!(history.exit_code, Some(0_i32), "{history:?}");
    assert!(history.stdout.contains("EXIT_ARMED"), "{history:?}");
}
#[test]
fn close_current_from_nested_shell_targets_its_own_tab() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let accepted = run(&target.tab_id, SHELL, 0.0);
    wait_for_completion(&accepted);
    #[cfg(windows)]
    let command = "& $env:FUNCTERM_HELPER_EXECUTABLE close --current";
    #[cfg(unix)]
    let command = "\"$FUNCTERM_HELPER_EXECUTABLE\" close --current";
    let closed = run(&target.tab_id, command, 5.0);
    wait_for_completion(&closed);
    assert!(
        !tab_view(&target.tab_id).alive,
        "close --current response: {}",
        String::from_utf8_lossy(&closed.stdout)
    );
    assert!(tab_view(&other.tab_id).alive);
}
#[test]
fn failed_startup_removes_its_runtime_directory() {
    let guard = locked_with_env(&[("FUNCTERM_SHELL_LOAD_PROFILE", "true")]);
    let root = temp_root();
    let tab = create_tab(&root, "python");
    let workspace = tab_directory(&tab.tab_id);
    let tabs = workspace.parent().unwrap();
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
    assert!(tab_view(&tab.tab_id).alive);
}
