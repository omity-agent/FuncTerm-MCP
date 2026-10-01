use crate::support::{
    create_tab, locked_with_env, parse_command_id, parse_command_result, parse_tab_view, run_cli,
    run_cli_with_env, send_command, send_command_with_env, temp_root,
};
use core::time::Duration;
use std::sync::mpsc;
extern crate alloc;
use alloc::sync::Arc;
use std::sync::Barrier;
use std::thread;
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
fn close_busy_tab_releases_waiting_client() {
    let guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let env = guard.env();
    let tab_id = target.tab_id.clone();
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || {
        #[cfg(windows)]
        let command = "Start-Sleep -Seconds 60";
        #[cfg(unix)]
        let command = "sleep 60";
        sender
            .send(send_command_with_env(&env, &tab_id, command, 15.0))
            .unwrap();
    });
    for attempt in 0_u8..100 {
        let view = run_cli(&["view", &target.tab_id]);
        assert!(view.status.success());
        if String::from_utf8_lossy(&view.stdout).contains("<IDLE>\nfalse\n") {
            break;
        }
        assert!(attempt < 99, "command did not become busy");
        thread::sleep(Duration::from_millis(20));
    }
    let closed = run_cli(&["close", "--tab-id", &target.tab_id]);
    assert!(
        closed.status.success(),
        "{}",
        String::from_utf8_lossy(&closed.stderr)
    );
    let output = receiver.recv_timeout(Duration::from_secs(5)).unwrap();
    worker.join().unwrap();
    let command_id = parse_command_id(&output);
    let result = parse_command_result(&output);
    assert!(result.finished);
    assert_ne!(result.exit_code, Some(0_i32));
    assert!(parse_command_result(&run_cli(&["view", &command_id])).finished);
    assert!(!parse_tab_view(&run_cli(&["view", &target.tab_id])).alive);
}
#[test]
fn close_current_from_nested_shell_targets_its_own_tab() {
    let _guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    let other = create_tab(&temp_root(), SHELL);
    let nested = send_command(&target.tab_id, SHELL, 5.0);
    assert!(parse_command_result(&nested).finished);
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
fn close_rejects_unknown_id_and_missing_current_context() {
    let guard = locked_with_env(&[]);
    let unknown = run_cli(&["close", "--tab-id", "tab-does-not-exist"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown tab id"));
    let mut env = guard.env();
    env.push(("FUNCTERM_TAB_ID".to_owned(), String::new()));
    let outside = run_cli_with_env(&["close", "--current"], &env);
    assert!(!outside.status.success());
    assert!(String::from_utf8_lossy(&outside.stderr).contains("current Tab"));
}
#[test]
fn concurrent_command_views_share_completion_and_cancellation() {
    assert_shared_outcome(false);
    assert_shared_outcome(true);
}
fn assert_shared_outcome(close: bool) {
    let guard = locked_with_env(&[]);
    let target = create_tab(&temp_root(), SHELL);
    #[cfg(windows)]
    let command = format!(
        "Write-Output OBSERVER_BEGIN\nStart-Sleep -Seconds {}\nWrite-Output OBSERVER_END",
        if close { 60_u8 } else { 2_u8 }
    );
    #[cfg(unix)]
    let command = format!(
        "echo OBSERVER_BEGIN\nsleep {}\necho OBSERVER_END",
        if close { 60_u8 } else { 2_u8 }
    );
    let accepted = send_command(&target.tab_id, &command, 0.0);
    let command_id = parse_command_id(&accepted);
    let barrier = Arc::new(Barrier::new(9));
    let workers: [_; 8] = core::array::from_fn(|_| {
        let env = guard.env();
        let id = command_id.clone();
        let ready = Arc::clone(&barrier);
        thread::spawn(move || {
            ready.wait();
            let output = run_cli_with_env(&["view", &id, "--wait-timeout", "8"], &env);
            parse_command_result(&output)
        })
    });
    barrier.wait();
    if close {
        thread::sleep(Duration::from_millis(250));
        let closed = run_cli(&["close", "--tab-id", &target.tab_id]);
        assert!(closed.status.success(), "{closed:?}");
    }
    let settled = Instant::now();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert!(settled.elapsed() < Duration::from_secs(5));
    let cached = parse_command_result(&run_cli(&["view", &command_id]));
    for result in results {
        assert!(result.finished);
        assert_eq!(result.exit_code, cached.exit_code);
        assert_eq!(result.stdout, cached.stdout);
        assert_eq!(result.stderr, cached.stderr);
    }
    assert!(cached.finished);
    if close {
        assert_ne!(cached.exit_code, Some(0_i32));
        assert!(!parse_tab_view(&run_cli(&["view", &target.tab_id])).alive);
    } else {
        assert_eq!(cached.exit_code, Some(0_i32));
        assert_eq!(
            cached.stdout.split_whitespace().collect::<Vec<_>>(),
            ["OBSERVER_BEGIN", "OBSERVER_END"]
        );
        let tab = run_cli(&["view", &target.tab_id]);
        assert!(tab.status.success());
        assert!(String::from_utf8_lossy(&tab.stdout).contains("<IDLE>\ntrue\n"));
    }
}
