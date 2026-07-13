use crate::runtime::protocol::CommandSnapshot;
use crate::runtime::session::records::{
    CommandRecord, command_note, read_and_clear_command_result, read_command_result,
    remove_record_directory, wait_for_done, write_failed_result,
};
use crate::runtime::session::terminal::CommandTitle;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use std::sync::Mutex;
use std::time::Instant;
pub(in crate::engine::runtime::session::manager) struct ManagedCommand {
    id: String,
    record: CommandRecord,
    started_at: Instant,
    state: Mutex<CommandState>,
    cached_view: Mutex<Option<CommandSnapshot>>,
    title: Arc<CommandTitle>,
}
#[derive(Clone, Copy)]
enum CommandState {
    Running,
    Finished,
    Failed,
}
pub(in crate::engine::runtime::session::manager) enum CommandWait {
    Running,
    Finished,
    Failed,
}
impl ManagedCommand {
    pub(super) fn new(id: String, record: CommandRecord, title: Arc<CommandTitle>) -> Self {
        Self {
            id,
            record,
            started_at: Instant::now(),
            state: Mutex::new(CommandState::Running),
            cached_view: Mutex::new(None),
            title,
        }
    }
    pub(in crate::engine::runtime::session::manager) fn id(&self) -> &str {
        &self.id
    }
    pub(super) const fn record(&self) -> &CommandRecord {
        &self.record
    }
    pub(in crate::engine::runtime::session::manager) fn wait(
        &self,
        limit: Duration,
    ) -> Result<CommandWait> {
        match self.state()? {
            CommandState::Finished => return Ok(CommandWait::Finished),
            CommandState::Failed => return Ok(CommandWait::Failed),
            CommandState::Running => {}
        }
        match wait_for_done(&self.record.done, limit) {
            Ok(true) => Ok(CommandWait::Finished),
            Ok(false) => Ok(CommandWait::Running),
            Err(error) => {
                self.mark_failed(format!("failed to watch command completion: {error:#}"))?;
                Err(error)
            }
        }
    }
    pub(super) fn view(&self) -> Result<CommandSnapshot> {
        if let Some(view) = self.cached_view()? {
            return Ok(view);
        }
        read_command_result(&self.record, self.time_consumption(), self.title.current()?)
    }
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the state lock serializes the single read-and-delete of command files"
    )]
    pub(super) fn mark_finished(&self) -> Result<()> {
        let mut state = self.lock_state()?;
        if !matches!(*state, CommandState::Running) {
            self.lock_cached_view()?
                .as_ref()
                .context("finished command is missing cached view")?;
            return Ok(());
        }
        let title = self.title.wait_finished()?;
        let view = read_and_clear_command_result(&self.record, self.time_consumption(), title)?;
        *self.lock_cached_view()? = Some(view);
        *state = CommandState::Finished;
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn mark_failed(
        &self,
        message: impl Into<String>,
    ) -> Result<()> {
        let failure_message = message.into();
        let mut state = self.lock_state()?;
        if matches!(*state, CommandState::Running) {
            let title = self.title.cancel()?;
            write_failed_result(&self.id, &self.record, &failure_message)?;
            let view = failure_view(
                &self.record,
                &failure_message,
                self.time_consumption(),
                title,
            )?;
            if let Err(error) = remove_record_directory(&self.record) {
                eprintln!("{error:#}");
            }
            *self.lock_cached_view()? = Some(view);
            *state = CommandState::Failed;
        }
        drop(state);
        Ok(())
    }
    fn state(&self) -> Result<CommandState> {
        Ok(*self.lock_state()?)
    }
    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, CommandState>> {
        self.state
            .lock()
            .map_err(|error| anyhow::anyhow!("command state mutex poisoned: {error}"))
    }
    fn cached_view(&self) -> Result<Option<CommandSnapshot>> {
        Ok(self.lock_cached_view()?.clone())
    }
    fn lock_cached_view(&self) -> Result<std::sync::MutexGuard<'_, Option<CommandSnapshot>>> {
        self.cached_view
            .lock()
            .map_err(|error| anyhow::anyhow!("command view mutex poisoned: {error}"))
    }
    fn time_consumption(&self) -> Duration {
        self.started_at.elapsed()
    }
}
fn failure_view(
    record: &CommandRecord,
    message: &str,
    time_consumption: Duration,
    title: String,
) -> Result<CommandSnapshot> {
    let mut snapshot = read_command_result(record, time_consumption, title)?;
    if !snapshot.command.finished {
        snapshot.command.finished = true;
        snapshot.command.exit_code = Some(1_i32);
        snapshot.note = command_note(&snapshot.command.stdout, &snapshot.command.stderr, message);
    }
    Ok(snapshot)
}
