use crate::support::{
    create_tab, create_tab_with_env, locked_with_env, parse_command_result, parse_tab_view,
    run_cli, run_cli_with_env, send_command, temp_dir, temp_root,
};
use core::time::Duration;
use named_lock::NamedLock;
use std::thread;
#[cfg(windows)]
const SHELL: &str = "powershell";
#[cfg(unix)]
const SHELL: &str = "bash";
#[test]
fn restart_discards_old_tabs_without_stopping_another_service() {
    let first = locked_with_env(&[]);
    let first_environment = first.env();
    let previous = create_tab(&temp_root(), SHELL);
    let accepted = send_command(&previous.tab_id, "echo BEFORE_RESTART", 5.0);
    assert_eq!(parse_command_result(&accepted).exit_code, Some(0_i32));
    let second = locked_with_env(&[]);
    let other = create_tab(&temp_root(), SHELL);
    let restarted = run_cli_with_env(&["daemon", "restart"], &first_environment);
    assert!(restarted.status.success(), "{restarted:?}");
    let unknown = run_cli_with_env(&["view", &previous.tab_id], &first_environment);
    assert!(!unknown.status.success(), "{unknown:?}");
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown id"));
    assert!(parse_tab_view(&run_cli(&["view", &other.tab_id])).alive);
    let fresh = create_tab_with_env(&temp_root(), SHELL, &first_environment);
    let recovered = run_cli_with_env(
        &[
            "send-command",
            &fresh.tab_id,
            "--command",
            "echo AFTER_RESTART",
            "--separate-output",
            "true",
            "--wait-timeout",
            "5",
        ],
        &first_environment,
    );
    let result = parse_command_result(&recovered);
    assert_eq!(result.exit_code, Some(0_i32));
    assert!(result.stdout.contains("AFTER_RESTART"));
    drop(second);
    drop(first);
}
#[test]
fn concurrent_starts_share_a_single_background_instance() {
    let guard = locked_with_env(&[]);
    assert_lifecycle("stop", "stopped");
    let workers: [_; 6] = core::array::from_fn(|_| {
        let environment = guard.env();
        thread::spawn(move || run_cli_with_env(&["daemon", "start"], &environment))
    });
    let outputs: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    for output in &outputs {
        assert!(output.status.success(), "{output:?}");
    }
    assert_eq!(
        outputs
            .iter()
            .filter(|output| String::from_utf8_lossy(&output.stdout).trim() == "daemon started")
            .count(),
        1
    );
    let tab = create_tab(&temp_root(), SHELL);
    assert!(parse_tab_view(&run_cli(&["view", &tab.tab_id])).alive);
    assert_lifecycle("start", "already running");
    assert_lifecycle("stop", "stopped");
    assert_lifecycle("stop", "not running");
    assert_lifecycle("restart", "restarted");
    let restarted = create_tab(&temp_root(), SHELL);
    assert!(parse_tab_view(&run_cli(&["view", &restarted.tab_id])).alive);
}
#[test]
fn stop_terminates_running_commands_and_allows_automatic_startup() {
    let guard = locked_with_env(&[]);
    let marker = temp_dir("stopped-command").join("unexpected-completion.txt");
    let tab = create_tab(&temp_root(), SHELL);
    #[cfg(windows)]
    let command = format!(
        "Start-Sleep -Seconds 2\nSet-Content -LiteralPath {} -Value UNEXPECTED_COMPLETION",
        functerm::shell::quote::powershell_string(marker.to_str().unwrap()),
    );
    #[cfg(unix)]
    let command = format!(
        "sleep 2\nprintf '%s\\n' UNEXPECTED_COMPLETION > {}",
        functerm::shell::quote::posix_string(marker.to_str().unwrap()),
    );
    let running = parse_command_result(&send_command(&tab.tab_id, &command, 0.0));
    assert!(!running.finished);
    assert_lifecycle("stop", "stopped");
    assert_stopped(&guard.env());
    thread::sleep(Duration::from_secs(3));
    assert!(!marker.exists(), "stopped command continued running");
    let fresh = create_tab(&temp_root(), SHELL);
    assert!(parse_tab_view(&run_cli(&["view", &fresh.tab_id])).alive);
    drop(guard);
}
fn assert_lifecycle(command: &str, expected: &str) {
    let output = run_cli(&["daemon", command]);
    assert!(output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stdout).contains(expected),
        "{output:?}"
    );
}
fn assert_stopped(environment: &[(String, String)]) {
    let service = &environment
        .iter()
        .find(|pair| pair.0 == "FUNCTERM_DAEMON_SERVICE_NAME")
        .unwrap()
        .1;
    let name = format!(
        "functerm-instance-{}",
        blake3::hash(service.as_bytes()).to_hex()
    );
    let lock = NamedLock::create(&name).unwrap();
    let _guard = lock
        .try_lock()
        .unwrap_or_else(|error| panic!("daemon instance lock is still held: {error}"));
}
