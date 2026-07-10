use crate::runtime::session::manager::shell_session::{
    KeyboardWriteFailure, ShellSession, ShellSessionParts,
};
use crate::runtime::session::terminal::{TerminalParser, TerminalWriter, start_reader};
use crate::shell::ShellChoice;
use alloc::sync::Arc;
use rmux_pty::{ChildCommand, TerminalSize as PtyTerminalSize};
use std::sync::{Barrier, Mutex};
use std::thread;
use tastty_core::TerminalSize;
#[test]
fn busy_state_rejects_conflicting_reservation() {
    let shell = test_shell(Some("command-current"));
    let error = shell.reserve("command-next").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("shell is busy with command command-current")
    );
    assert_eq!(
        shell.busy_command_id().unwrap().as_deref(),
        Some("command-current")
    );
}
#[test]
fn busy_state_release_keeps_conflicting_owner() {
    let shell = test_shell(Some("command-owner"));
    shell.release("command-stranger").unwrap();
    assert_eq!(
        shell.busy_command_id().unwrap().as_deref(),
        Some("command-owner")
    );
}
#[test]
fn busy_state_allows_only_one_concurrent_reservation() {
    const ATTEMPTS: usize = 16;
    let shell = Arc::new(test_shell(None));
    let barrier = Arc::new(Barrier::new(ATTEMPTS));
    let mut workers = Vec::with_capacity(ATTEMPTS);
    for index in 0..ATTEMPTS {
        let worker_shell = Arc::clone(&shell);
        let worker_barrier = Arc::clone(&barrier);
        workers.push(thread::spawn(move || {
            let command_id = format!("command-{index}");
            worker_barrier.wait();
            worker_shell.reserve(&command_id).map(|()| command_id)
        }));
    }
    let mut reserved_ids = Vec::new();
    let mut conflict_count = 0;
    for worker in workers {
        match worker.join().unwrap() {
            Ok(command_id) => reserved_ids.push(command_id),
            Err(error) => {
                assert!(error.to_string().contains("shell is busy with command"));
                conflict_count += 1;
            }
        }
    }
    assert_eq!(reserved_ids.len(), 1);
    assert_eq!(conflict_count, ATTEMPTS - 1);
    let reserved_id = reserved_ids.first().unwrap();
    assert_eq!(
        shell.busy_command_id().unwrap().as_deref(),
        Some(reserved_id.as_str())
    );
}
#[test]
fn manual_write_rejects_idle_prompt() {
    let shell = test_shell(None);
    let error = shell
        .write_keyboard_for_running_command(b"typed")
        .unwrap_err();
    assert!(matches!(error, KeyboardWriteFailure::IdlePrompt));
}
#[test]
fn manual_write_allows_running_command() {
    let shell = test_shell(Some("command-current"));
    shell.write_keyboard_for_running_command(b"typed").unwrap();
}
fn test_shell(busy: Option<&str>) -> ShellSession {
    let spawned = test_child_command()
        .size(PtyTerminalSize::new(120, 30))
        .spawn()
        .unwrap();
    let (master, child) = spawned.into_parts();
    let writer: TerminalWriter = Arc::new(Mutex::new(master.try_clone_io().unwrap()));
    let screen = Arc::new(Mutex::new(TerminalParser::new(
        TerminalSize {
            rows: 30,
            cols: 120,
        },
        0,
    )));
    let reader = start_reader(Arc::clone(&screen), Arc::clone(&writer), master.into_io()).unwrap();
    ShellSession::new(ShellSessionParts {
        choice: ShellChoice::PowerShell,
        cwd: crate::test_fs::temp_root(),
        writer,
        screen,
        busy: busy.map(str::to_owned),
        command_root: crate::test_fs::temp_case("command-manager"),
        active_shell_file: crate::test_fs::temp_case("command-manager-active")
            .join("active-shell.txt"),
        command_start_timeout: core::time::Duration::from_secs(1),
        child,
        reader: Some(reader),
    })
}
#[cfg(windows)]
fn test_child_command() -> ChildCommand {
    ChildCommand::new("cmd.exe").args(["/D", "/Q"])
}
#[cfg(not(windows))]
fn test_child_command() -> ChildCommand {
    ChildCommand::new("sh")
}
