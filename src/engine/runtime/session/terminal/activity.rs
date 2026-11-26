use super::Terminal;
use crate::runtime::session::observation::wait_until;
use anyhow::{Result, bail};
use core::time::Duration;
impl Terminal {
    pub(in crate::engine::runtime::session) fn visible_revision(&self) -> Result<u64> {
        let state = self.state.lock();
        if let Some(message) = state.reader_failure.as_deref() {
            bail!("terminal reader is unavailable: {message}");
        }
        if state.reader_closed {
            bail!("terminal reader is closed");
        }
        Ok(state.visible_revision)
    }
    pub(in crate::engine::runtime::session) async fn wait_for_visible_change(
        &self,
        visible_revision: u64,
        wait_timeout: Duration,
    ) -> Result<()> {
        if wait_timeout.is_zero() {
            return Ok(());
        }
        wait_until(&self.changed, wait_timeout, || {
            let state = self.state.lock();
            if let Some(message) = state.reader_failure.as_deref() {
                bail!("terminal reader failed while waiting for output: {message}");
            }
            Ok(state.visible_revision != visible_revision
                || state.reader_closed
                || state.process_exited)
        })
        .await?;
        Ok(())
    }
    pub(in crate::engine::runtime::session) fn process_exited(&self) {
        self.state.lock().process_exited = true;
        self.lifetime_changed.notify(usize::MAX);
        self.changed.notify(usize::MAX);
    }
    pub(in crate::engine::runtime::session) fn process_failed(&self, message: String) {
        self.state.lock().process_failure = Some(message);
        self.lifetime_changed.notify(usize::MAX);
    }
    pub(in crate::engine::runtime::session) async fn wait_for_exit(&self) -> Result<bool> {
        wait_until(&self.lifetime_changed, Duration::MAX, || {
            let state = self.state.lock();
            if state.session_stopped {
                return Ok(true);
            }
            if let Some(message) = state.process_failure.as_deref() {
                bail!("{message}");
            }
            Ok(state.process_exited)
        })
        .await?;
        Ok(!self.state.lock().session_stopped)
    }
    pub(in crate::engine::runtime::session) fn session_stopped(&self) {
        self.state.lock().session_stopped = true;
        self.lifetime_changed.notify(usize::MAX);
        self.changed.notify(usize::MAX);
    }
    pub(in crate::engine::runtime::session) async fn wait_for_shutdown(&self) -> Result<()> {
        wait_until(&self.lifetime_changed, Duration::MAX, || {
            Ok(self.state.lock().session_stopped)
        })
        .await?;
        Ok(())
    }
    pub(in crate::engine::runtime::session) fn reader_closed(&self) {
        let mut state = self.state.lock();
        if let Err(error) = state.captures.freeze() {
            let message = format!("failed to finish closed terminal capture: {error:#}");
            state.captures.fail(&message);
            state.reader_failure = Some(message);
        }
        state.reader_closed = true;
        drop(state);
        self.changed.notify(usize::MAX);
    }
    pub(in crate::engine::runtime::session) fn reader_failed(&self, message: &str) {
        let mut state = self.state.lock();
        state.captures.fail(message);
        state.reader_closed = true;
        state.reader_failure = Some(message.to_owned());
        drop(state);
        self.changed.notify(usize::MAX);
    }
}
