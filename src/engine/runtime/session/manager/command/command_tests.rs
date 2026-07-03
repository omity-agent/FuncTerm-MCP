use crate::runtime::session::manager::shell_session::{
    KeyboardWriteFailure, ShellSession, ShellSessionParts,
};
use crate::runtime::session::terminal::{TerminalCallbacks, TerminalParser};
use crate::shell::ShellChoice;
use alloc::sync::Arc;
use anyhow::Error;
use portable_pty::{Child, ChildKiller, CommandBuilder, ExitStatus, SlavePty};
use std::io::{Result as IoResult, Write};
use std::sync::{Barrier, Mutex};
use std::thread;
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
    let writer: Box<dyn Write + Send> = Box::<Vec<u8>>::default();
    let child: Box<dyn Child + Send + Sync> = Box::new(TestChild);
    let slave: Box<dyn SlavePty + Send> = Box::new(TestSlave);
    ShellSession::new(ShellSessionParts {
        choice: ShellChoice::PowerShell,
        cwd: std::env::temp_dir(),
        writer: Arc::new(Mutex::new(writer)),
        screen: Arc::new(Mutex::new(TerminalParser::new_with_callbacks(
            30,
            120,
            0,
            TerminalCallbacks::default(),
        ))),
        busy: busy.map(str::to_owned),
        command_root: std::env::temp_dir().join("functerm-test-commands"),
        active_shell_file: std::env::temp_dir()
            .join("functerm-test-commands")
            .join("active-shell.txt"),
        command_start_timeout: core::time::Duration::from_secs(1),
        process_tree: crate::runtime::session::manager::process_tree::ProcessTree::new(),
        child,
        slave,
    })
}
#[derive(Debug)]
struct TestChild;
impl ChildKiller for TestChild {
    fn kill(&mut self) -> IoResult<()> {
        Ok(())
    }
    fn clone_killer(&self) -> Box<dyn ChildKiller + Send + Sync> {
        Box::new(Self)
    }
}
impl Child for TestChild {
    fn try_wait(&mut self) -> IoResult<Option<ExitStatus>> {
        Ok(Some(ExitStatus::with_exit_code(0)))
    }
    fn wait(&mut self) -> IoResult<ExitStatus> {
        Ok(ExitStatus::with_exit_code(0))
    }
    fn process_id(&self) -> Option<u32> {
        None
    }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> {
        None
    }
}
struct TestSlave;
impl SlavePty for TestSlave {
    fn spawn_command(
        &self,
        _command: CommandBuilder,
    ) -> Result<Box<dyn Child + Send + Sync>, Error> {
        anyhow::bail!("test slave cannot spawn commands")
    }
}
