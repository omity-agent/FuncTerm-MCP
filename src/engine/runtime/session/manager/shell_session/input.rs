use super::super::command::ManagedCommand;
use super::ShellSession;
use crate::runtime::protocol::{KeyboardInput, RequestRejected};
use crate::runtime::session::keyboard::{InputBatch, InputDelivery};
use alloc::{borrow::Cow, sync::Arc};
use anyhow::{Context as _, Result};
use std::io::Write as _;
impl ShellSession {
    pub(in crate::engine::runtime::session::manager) fn write_keyboard_for_running_command(
        &self,
        expected: &Arc<ManagedCommand>,
        input: &KeyboardInput,
    ) -> Result<u64> {
        if !self.holds_command(expected) {
            return Err(RequestRejected::new(
                "manual_write target command ended before input could be written",
            )
            .into());
        }
        let command_id = expected.id();
        tracing::debug!(%command_id, "delivering keyboard input to running command");
        let visible_revision = self.screen.visible_revision()?;
        if self.resuming() && expected.capture_finished()? {
            self.write_keyboard(command_id, input)?;
        } else {
            expected.deliver_input(|| self.write_keyboard(command_id, input))?;
        }
        tracing::debug!(%command_id, "keyboard input delivery completed");
        Ok(visible_revision)
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow the request payload without cloning its text or bytes"
    )]
    fn write_keyboard(&self, command_id: &str, input: &KeyboardInput) -> Result<InputDelivery> {
        let shell_bytes = match input {
            KeyboardInput::Text(text) => self.current_choice().keyboard_bytes(text.as_bytes()),
            KeyboardInput::Bytes(bytes) => Cow::Borrowed(bytes.as_slice()),
        };
        let batch = InputBatch::from_bytes(&shell_bytes);
        let delivery = batch.delivery();
        if delivery.interrupted() {
            self.screen.defer_window_titles(command_id);
        }
        let mut writer = self.writer.lock();
        for segment in batch.segments() {
            writer
                .write_all(segment)
                .context("failed to write to pty")?;
        }
        writer.flush().context("failed to flush pty writer")?;
        drop(writer);
        Ok(delivery)
    }
    pub(in crate::engine::runtime::session::manager) fn ensure_idle(
        &self,
        tab_id: &str,
    ) -> Result<()> {
        if let Some(existing) = self.busy.lock().as_ref() {
            return Err(RequestRejected::new(format!(
                "The command was not executed because `{tab_id}` is busy with `{}`",
                existing.id()
            ))
            .into());
        }
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn reserve(
        &self,
        command: Arc<ManagedCommand>,
    ) {
        *self.busy.lock() = Some(command);
    }
    pub(in crate::engine::runtime::session::manager) fn release(&self, command_id: &str) {
        let mut busy = self.busy.lock();
        if busy.as_ref().map(|command| command.id()) == Some(command_id) {
            *busy = None;
        }
    }
    pub(in crate::engine::runtime::session::manager) fn is_busy(&self) -> bool {
        self.busy.lock().is_some() || self.resuming()
    }
    pub(in crate::engine::runtime::session::manager) fn holds_command(
        &self,
        expected: &Arc<ManagedCommand>,
    ) -> bool {
        self.busy
            .lock()
            .as_ref()
            .is_some_and(|command| Arc::ptr_eq(command, expected))
    }
    pub(in crate::engine::runtime::session::manager) fn busy_command(
        &self,
    ) -> Option<Arc<ManagedCommand>> {
        self.busy.lock().clone()
    }
}
