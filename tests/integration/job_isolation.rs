use crate::support::ChildGuard;
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

fn waiting_process() -> Command {
    let mut command = Command::new(std::env::var_os("COMSPEC").unwrap());
    command
        .args(["/D", "/Q", "/C", "set /p INPUT="])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}
