use crate::support::{
    ChildGuard, apply_test_environment, create_tab, locked_with_env, parse_tab_view, run_cli,
    temp_root,
};
use std::process::{Command, Stdio};
#[test]
fn terminating_owned_job_preserves_sibling_process() {
    let mut target = ChildGuard::spawn(&mut waiting_process()).unwrap();
    let mut unrelated = ChildGuard::spawn(&mut waiting_process()).unwrap();
    target.terminate();
    assert!(target.child_mut().try_wait().unwrap().is_some());
    assert!(unrelated.child_mut().try_wait().unwrap().is_none());
    unrelated.terminate();
    assert!(unrelated.child_mut().try_wait().unwrap().is_some());
}
#[test]
fn background_daemon_survives_its_launcher_job() {
    let guard = locked_with_env(&[]);
    let stopped = run_cli(&["daemon", "stop"]);
    assert!(stopped.status.success(), "{stopped:?}");
    let mut command = Command::new(env!("CARGO_BIN_EXE_functerm"));
    command
        .args(["daemon", "start"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    apply_test_environment(&mut command, &guard.env());
    let mut launcher = ChildGuard::spawn(&mut command).unwrap();
    let status = launcher.child_mut().wait().unwrap();
    assert!(status.success(), "{status}");
    let tab = create_tab(&temp_root(), "powershell");
    launcher.terminate();
    assert!(parse_tab_view(&run_cli(&["view", &tab.tab_id])).alive);
    drop(guard);
}
fn waiting_process() -> Command {
    let mut command = Command::new(std::env::var_os("COMSPEC").unwrap());
    command
        .args(["/D", "/Q", "/C", "set /p INPUT="])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
