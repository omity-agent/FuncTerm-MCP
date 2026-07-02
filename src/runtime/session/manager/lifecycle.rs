use super::ShellSession;
use crate::runtime::session::support::lock_mutex;
use anyhow::{Result, bail};
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            eprintln!("failed to terminate shell process tree during cleanup: {error}");
        }
        if let Ok(mut child) = self.child.lock() {
            match child.try_wait() {
                Ok(Some(_)) => {}
                Ok(None) => {
                    if let Err(error) = child.kill() {
                        eprintln!("failed to kill shell child during cleanup: {error}");
                    }
                    if let Err(error) = child.wait() {
                        eprintln!("failed to wait shell child during cleanup: {error}");
                    }
                }
                Err(error) => eprintln!("failed to poll shell child during cleanup: {error}"),
            }
        }
    }
}
pub(super) fn reserve_shell(shell: &ShellSession, command_id: &str) -> Result<()> {
    {
        let mut busy = lock_mutex(&shell.busy, "busy")?;
        if let Some(existing_id) = busy.as_deref() {
            bail!("shell is busy with command {existing_id}");
        }
        *busy = Some(command_id.to_owned());
    }
    Ok(())
}
pub(super) fn release_shell(shell: &ShellSession, command_id: &str) -> Result<()> {
    {
        let mut busy = lock_mutex(&shell.busy, "busy")?;
        if busy.as_deref() == Some(command_id) {
            *busy = None;
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::{release_shell, reserve_shell};
    use crate::runtime::session::manager::ShellSession;
    use crate::runtime::session::support::lock_mutex;
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
        let error = reserve_shell(&shell, "command-next").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("shell is busy with command command-current")
        );
        let busy = lock_mutex(&shell.busy, "busy").unwrap();
        assert_eq!(busy.as_deref(), Some("command-current"));
    }
    #[test]
    fn busy_state_release_keeps_conflicting_owner() {
        let shell = test_shell(Some("command-owner"));
        release_shell(&shell, "command-stranger").unwrap();
        let busy = lock_mutex(&shell.busy, "busy").unwrap();
        assert_eq!(busy.as_deref(), Some("command-owner"));
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
                reserve_shell(&worker_shell, &command_id).map(|()| command_id)
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
        let busy = lock_mutex(&shell.busy, "busy").unwrap();
        assert_eq!(busy.as_deref(), Some(reserved_id.as_str()));
    }
    fn test_shell(busy: Option<&str>) -> ShellSession {
        let writer: Box<dyn Write + Send> = Box::<Vec<u8>>::default();
        let child: Box<dyn Child + Send + Sync> = Box::new(TestChild);
        let slave: Box<dyn SlavePty + Send> = Box::new(TestSlave);
        ShellSession {
            choice: Mutex::new(ShellChoice::PowerShell),
            cwd: Mutex::new(std::env::temp_dir()),
            writer: Arc::new(Mutex::new(writer)),
            screen: Arc::new(Mutex::new(vt100::Parser::new(30, 120, 0))),
            busy: Mutex::new(busy.map(str::to_owned)),
            command_root: std::env::temp_dir().join("functerm-test-commands"),
            active_shell_file: std::env::temp_dir()
                .join("functerm-test-commands")
                .join("active-shell.txt"),
            process_tree: crate::runtime::session::manager::process_tree::ProcessTree::new()
                .unwrap(),
            child: Mutex::new(child),
            _slave: Mutex::new(slave),
        }
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
}
