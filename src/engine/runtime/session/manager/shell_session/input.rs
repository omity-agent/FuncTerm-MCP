use super::ShellSession;
use crate::runtime::protocol::KeyboardInput;
use crate::runtime::session::keyboard::{self, InputBatch, InputDelivery};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use std::io::Write as _;
#[derive(Debug, thiserror :: Error)]
pub(in crate::engine::runtime::session::manager) enum KeyboardWriteFailure {
    #[error(
        "manual_write is unavailable while the prompt is idle; use send_command for prompt commands"
    )]
    IdlePrompt,
    #[error("manual_write target command ended before input could be written")]
    CommandEnded,
    #[error(transparent)]
    Write(#[from] anyhow::Error),
}
impl ShellSession {
    pub(in crate::engine::runtime::session::manager) fn write_keyboard_for_running_command(
        &self,
        input: KeyboardInput,
        waiting: Duration,
    ) -> Result<(), KeyboardWriteFailure> {
        let busy = self.busy.lock();
        let Some(command) = busy.as_ref() else {
            return Err(KeyboardWriteFailure::IdlePrompt);
        };
        let revision = self.screen.output_revision()?;
        let write_result = command.deliver_input(|| self.write_keyboard(input));
        drop(busy);
        match write_result {
            Ok(()) => {}
            Err(super::super::command::CommandInputFailure::CommandEnded) => {
                return Err(KeyboardWriteFailure::CommandEnded);
            }
            Err(super::super::command::CommandInputFailure::Write(error)) => {
                return Err(KeyboardWriteFailure::Write(error));
            }
        }
        self.screen.wait_for_output(revision, waiting)?;
        Ok(())
    }
    fn write_keyboard(&self, input: KeyboardInput) -> Result<InputDelivery> {
        let shell_bytes = match input {
            KeyboardInput::Text(text) => self
                .current_choice()
                .keyboard_bytes(text.as_bytes())
                .into_owned(),
            KeyboardInput::Bytes(bytes) => bytes,
        };
        let batch = InputBatch::from_bytes(&shell_bytes);
        let mut writer = self.writer.lock();
        for event in batch.events() {
            writer
                .write_all(keyboard::user_bytes(event))
                .context("failed to write to pty")?;
        }
        writer.flush().context("failed to flush pty writer")?;
        drop(writer);
        Ok(batch.delivery())
    }
    pub(in crate::engine::runtime::session::manager) fn reserve(
        &self,
        tab_id: &str,
        command: Arc<super::super::command::ManagedCommand>,
    ) -> Result<()> {
        let mut busy = self.busy.lock();
        if let Some(existing) = busy.as_ref() {
            anyhow::bail!(
                "The command was not executed because `{tab_id}` is busy with `{}`",
                existing.id()
            );
        }
        *busy = Some(command);
        drop(busy);
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn release(&self, command_id: &str) {
        let mut busy = self.busy.lock();
        if busy.as_ref().map(|command| command.id()) == Some(command_id) {
            *busy = None;
        }
    }
    pub(in crate::engine::runtime::session::manager) fn busy_command_id(&self) -> Option<String> {
        self.busy
            .lock()
            .as_ref()
            .map(|command| command.id().to_owned())
    }
}
