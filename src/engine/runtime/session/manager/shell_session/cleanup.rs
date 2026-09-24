use super::ShellSession;
use crate::runtime::session::manager::process;
use anyhow::{Context as _, Result};
impl ShellSession {
    pub(in crate::engine::runtime::session::manager) fn terminate(&self) -> Result<()> {
        let mut child = self.child.lock();
        self.process_tree.terminate()?;
        child.wait().context("failed to wait for closed shell")?;
        drop(child);
        drop(self.slave.lock().take());
        Ok(())
    }
}
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            eprintln!("failed to terminate shell process tree during cleanup: {error}");
        }
        let child = self.child.get_mut();
        process::cleanup(child.as_mut(), "shell child during cleanup");
        let slave = self.slave.get_mut();
        drop(slave.take());
        if let Some(reader) = self.reader.take() {
            process::join_reader(reader, "pty reader thread");
        }
    }
}
