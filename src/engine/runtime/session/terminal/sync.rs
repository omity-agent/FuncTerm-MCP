use super::Terminal;
use anyhow::{Result, bail};
use core::time::Duration;
impl Terminal {
    pub(in crate::engine::runtime::session) fn output_revision(&self) -> Result<u64> {
        let state = self.lock()?;
        if let Some(message) = state.reader_failure.as_deref() {
            bail!("terminal reader is unavailable: {message}");
        }
        if state.reader_closed {
            bail!("terminal reader is closed");
        }
        Ok(state.revision)
    }
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the terminal state guard is required by Condvar while waiting for a revision"
    )]
    pub(in crate::engine::runtime::session) fn wait_for_output(
        &self,
        revision: u64,
        waiting: Duration,
    ) -> Result<()> {
        if waiting.is_zero() {
            return Ok(());
        }
        let state = self.lock()?;
        let (waited_state, _timeout) = self
            .changed
            .wait_timeout_while(state, waiting, |current| {
                current.revision == revision
                    && !current.reader_closed
                    && current.reader_failure.is_none()
            })
            .map_err(|error| anyhow::anyhow!("terminal mutex poisoned while waiting: {error}"))?;
        if let Some(message) = waited_state.reader_failure.as_deref() {
            bail!("terminal reader failed while waiting for output: {message}");
        }
        drop(waited_state);
        Ok(())
    }
    pub(in crate::engine::runtime::session) fn reader_closed(&self) {
        match self.lock() {
            Ok(mut state) => {
                let message = "PTY reader closed before command title capture completed";
                state.captures.fail_all(message);
                state.reader_closed = true;
                drop(state);
                self.changed.notify_all();
            }
            Err(error) => eprintln!("failed to close command title captures: {error:#}"),
        }
    }
    pub(in crate::engine::runtime::session) fn reader_failed(&self, message: &str) {
        match self.lock() {
            Ok(mut state) => {
                state.captures.fail_all(message);
                state.reader_closed = true;
                state.reader_failure = Some(message.to_owned());
                drop(state);
                self.changed.notify_all();
            }
            Err(error) => eprintln!("failed to report terminal reader failure: {error:#}"),
        }
    }
}
