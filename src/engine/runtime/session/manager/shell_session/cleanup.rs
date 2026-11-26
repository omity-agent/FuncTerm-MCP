use super::ShellSession;
use crate::runtime::session::manager::process;
use anyhow::{Context as _, Result};
impl ShellSession {
    pub(in crate::engine::runtime::session::manager) fn terminate(&self) -> Result<()> {
        let mut terminated = self.terminated.lock();
        if *terminated {
            return Ok(());
        }
        let mut child = self.child.lock();
        self.process_tree.terminate()?;
        tracing::debug!("waiting for closed shell process");
        child.wait().context("failed to wait for closed shell")?;
        tracing::debug!("closed shell process exited");
        drop(child);
        drop(self.slave.lock().take());
        let joined = self.reader.lock().take().map_or(Ok(()), |reader| {
            reader
                .join()
                .map_err(|_panic| anyhow::anyhow!("PTY reader thread panicked"))
        });
        *terminated = true;
        drop(terminated);
        self.screen.session_stopped();
        joined?;
        Ok(())
    }
}
impl Drop for ShellSession {
    fn drop(&mut self) {
        if let Err(error) = self.process_tree.terminate() {
            tracing::error!(%error, "failed to terminate shell process tree during cleanup");
        }
        let child = self.child.get_mut();
        process::cleanup(child.as_mut(), "shell child during cleanup");
        let slave = self.slave.get_mut();
        drop(slave.take());
        if let Some(reader) = self.reader.get_mut().take() {
            tracing::debug!("joining closed shell PTY reader");
            process::join_reader(reader, "pty reader thread");
            tracing::debug!("closed shell PTY reader joined");
        }
        let state = self.tab_root.join(crate::contract::SESSION_STATE_DIRECTORY);
        for directory in [self.command_root.as_path(), state.as_path()] {
            if let Err(error) = self.workspace_watch.unwatch(directory) {
                tracing::error!("failed to release Tab workspace watch");
                tracing::debug!(
                    error = format!("{error:#}"),
                    "Tab workspace watch cleanup failed"
                );
            }
        }
        if let Some(directory) = self.directory.take()
            && let Err(error) = directory.close()
        {
            tracing::error!("failed to remove closed Tab workspace");
            tracing::debug!(%error, "Tab workspace cleanup failed");
        }
    }
}
