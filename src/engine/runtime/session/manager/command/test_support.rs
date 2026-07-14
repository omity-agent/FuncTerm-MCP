use crate::runtime::session::manager::shell_session::{ShellSession, ShellSessionParts};
use crate::runtime::session::terminal::Terminal;
use crate::shell::ShellChoice;
use alloc::sync::Arc;
use portable_pty::{Child, ChildKiller, ExitStatus};
use std::io::{Result as IoResult, Write};
use std::sync::{Mutex, mpsc};
use tastty_core::TerminalSize;
pub(super) fn test_shell(busy: Option<&str>) -> ShellSession {
    test_shell_with_writer(busy).0
}
pub(super) fn test_shell_with_writer(busy: Option<&str>) -> (ShellSession, Arc<Mutex<Vec<u8>>>) {
    let (shell, written, _terminal) = build_shell(busy, None);
    (shell, written)
}
pub(super) fn test_shell_with_flush(
    busy: Option<&str>,
) -> (ShellSession, Arc<Terminal>, mpsc::Receiver<()>) {
    let (flushed_tx, flushed_rx) = mpsc::channel();
    let (shell, _written, terminal) = build_shell(busy, Some(flushed_tx));
    (shell, terminal, flushed_rx)
}
fn build_shell(
    busy: Option<&str>,
    flushed: Option<mpsc::Sender<()>>,
) -> (ShellSession, Arc<Mutex<Vec<u8>>>, Arc<Terminal>) {
    let written = Arc::new(Mutex::new(Vec::new()));
    let writer: Box<dyn Write + Send> = Box::new(TestWriter {
        written: Arc::clone(&written),
        flushed,
    });
    let terminal = Arc::new(
        Terminal::new(
            TerminalSize {
                rows: 30,
                cols: 120,
            },
            0,
            "FuncTerm",
        )
        .unwrap(),
    );
    let shell = ShellSession::new(ShellSessionParts {
        choice: ShellChoice::PowerShell,
        cwd: crate::test_fs::temp_root(),
        writer: Arc::new(Mutex::new(writer)),
        screen: Arc::clone(&terminal),
        busy: busy.map(str::to_owned),
        command_root: crate::test_fs::temp_dir("command-manager"),
        active_shell_file: crate::test_fs::temp_dir("command-manager-active")
            .join("active-shell.txt"),
        command_start_timeout: core::time::Duration::from_secs(1),
        process_tree: crate::runtime::session::manager::process_tree::ProcessTree::new(),
        child: Box::new(TestChild),
        slave: None,
        reader: None,
    });
    (shell, written, terminal)
}
struct TestWriter {
    written: Arc<Mutex<Vec<u8>>>,
    flushed: Option<mpsc::Sender<()>>,
}
impl Write for TestWriter {
    fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
        self.written
            .lock()
            .map_err(|error| std::io::Error::other(error.to_string()))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> IoResult<()> {
        if let Some(sender) = self.flushed.take() {
            sender
                .send(())
                .map_err(|error| std::io::Error::other(error.to_string()))?;
        }
        Ok(())
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
