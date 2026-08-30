use crate::runtime::protocol::KeyboardInput;
use crate::runtime::session::manager::shell_session::KeyboardWriteFailure;
use alloc::sync::Arc;
use std::sync::{Barrier, mpsc};
use std::thread;
#[path = "test_support.rs"]
mod support;
use support::{test_shell, test_shell_with_flush, test_shell_with_writer};
#[test]
fn busy_state_rejects_conflicting_reservation() {
    let shell = test_shell(Some("command-current"));
    let error = shell.reserve("tab-current", "command-next").unwrap_err();
    assert_eq!(
        error.to_string(),
        "The command was not executed because `tab-current` is busy with `command-current`"
    );
    assert_eq!(shell.busy_command_id().as_deref(), Some("command-current"));
}
#[test]
fn busy_state_release_keeps_conflicting_owner() {
    let shell = test_shell(Some("command-owner"));
    shell.release("command-stranger");
    assert_eq!(shell.busy_command_id().as_deref(), Some("command-owner"));
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
            worker_shell
                .reserve("tab-current", &command_id)
                .map(|()| command_id)
        }));
    }
    let mut reserved_ids = Vec::new();
    let mut conflict_count = 0;
    for worker in workers {
        match worker.join().unwrap() {
            Ok(command_id) => reserved_ids.push(command_id),
            Err(error) => {
                assert!(error.to_string().contains("`tab-current` is busy"));
                conflict_count += 1;
            }
        }
    }
    assert_eq!(reserved_ids.len(), 1);
    assert_eq!(conflict_count, ATTEMPTS - 1);
    let reserved_id = reserved_ids.first().unwrap();
    assert_eq!(
        shell.busy_command_id().as_deref(),
        Some(reserved_id.as_str())
    );
}
#[test]
fn manual_write_rejects_idle_prompt() {
    let shell = test_shell(None);
    let error = shell
        .write_keyboard_for_running_command(
            KeyboardInput::Bytes(b"typed".to_vec()),
            core::time::Duration::ZERO,
        )
        .unwrap_err();
    assert!(matches!(error, KeyboardWriteFailure::IdlePrompt));
}
#[test]
fn manual_write_allows_running_command() {
    let shell = test_shell(Some("command-current"));
    shell
        .write_keyboard_for_running_command(
            KeyboardInput::Bytes(b"typed".to_vec()),
            core::time::Duration::ZERO,
        )
        .unwrap();
}
#[test]
fn raw_keyboard_bytes_skip_shell_text_normalization() {
    let (shell, written) = test_shell_with_writer(Some("command-current"));
    shell
        .write_keyboard_for_running_command(
            KeyboardInput::Bytes(b"line\n".to_vec()),
            core::time::Duration::ZERO,
        )
        .unwrap();
    assert_eq!(*written.lock(), b"line\n");
}
#[test]
fn keyboard_text_uses_active_shell_normalization() {
    let (shell, written) = test_shell_with_writer(Some("command-current"));
    shell
        .write_keyboard_for_running_command(
            KeyboardInput::Text("line\n".to_owned()),
            core::time::Duration::ZERO,
        )
        .unwrap();
    assert_eq!(*written.lock(), b"line\r");
}
#[test]
fn output_wait_does_not_hold_the_busy_state_lock() {
    let (shell, terminal, flushed) = test_shell_with_flush(Some("command-current"));
    let shared_shell = Arc::new(shell);
    let writing_shell = Arc::clone(&shared_shell);
    let writer = thread::spawn(move || {
        writing_shell.write_keyboard_for_running_command(
            KeyboardInput::Bytes(b"typed".to_vec()),
            core::time::Duration::from_secs(10),
        )
    });
    flushed
        .recv_timeout(core::time::Duration::from_secs(1))
        .unwrap();
    let releasing_shell = Arc::clone(&shared_shell);
    let (released_tx, released_rx) = mpsc::channel();
    let releaser = thread::spawn(move || {
        releasing_shell.release("command-current");
        released_tx.send(()).unwrap();
    });
    released_rx
        .recv_timeout(core::time::Duration::from_secs(1))
        .unwrap();
    terminal.reader_closed();
    writer.join().unwrap().unwrap();
    releaser.join().unwrap();
}
