use crate::runtime::protocol::ViewResult;
use crate::runtime::session::records::{
    CommandRecord, read_and_clear_command_result, read_command_result, remove_record_directory,
    wait_for_done, write_failed_result,
};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use std::sync::Mutex;
pub(in crate::runtime::session::manager) struct ManagedCommand {
    id: String,
    record: CommandRecord,
    state: Mutex<CommandState>,
    cached_view: Mutex<Option<ViewResult>>,
}
#[derive(Clone, Copy)]
enum CommandState {
    Running,
    Finished,
    Failed,
}
pub(in crate::runtime::session::manager) enum CommandWait {
    Running,
    Finished,
    Failed,
}
pub(super) struct ShellReservation {
    session: Arc<super::super::session::ShellSession>,
    command_id: String,
    released: bool,
}
impl ManagedCommand {
    pub(super) const fn new(id: String, record: CommandRecord) -> Self {
        Self {
            id,
            record,
            state: Mutex::new(CommandState::Running),
            cached_view: Mutex::new(None),
        }
    }
    pub(in crate::runtime::session::manager) fn id(&self) -> &str {
        &self.id
    }
    pub(super) const fn record(&self) -> &CommandRecord {
        &self.record
    }
    pub(in crate::runtime::session::manager) fn wait(
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
    pub(super) fn view(&self, fallback_cwd: &std::path::Path) -> Result<ViewResult> {
        if let Some(view) = self.cached_view()? {
            return Ok(view);
        }
        read_command_result(&self.record, fallback_cwd)
    }
    #[expect(
        clippy::significant_drop_tightening,
        reason = "the state lock serializes the single read-and-delete of command files"
    )]
    pub(super) fn mark_finished(&self, fallback_cwd: &std::path::Path) -> Result<ViewResult> {
        let mut state = self.lock_state()?;
        if !matches!(*state, CommandState::Running) {
            return self
                .cached_view()?
                .context("finished command is missing cached view");
        }
        let view = read_and_clear_command_result(&self.record, fallback_cwd)?;
        *self.lock_cached_view()? = Some(view.clone());
        *state = CommandState::Finished;
        Ok(view)
    }
    pub(in crate::runtime::session::manager) fn mark_failed(
        &self,
        message: impl Into<String>,
    ) -> Result<()> {
        let failure_message = message.into();
        let mut state = self.lock_state()?;
        if matches!(*state, CommandState::Running) {
            write_failed_result(&self.id, &self.record, &failure_message)?;
            let view = failure_view(&self.record, &failure_message)?;
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
    fn cached_view(&self) -> Result<Option<ViewResult>> {
        Ok(self.lock_cached_view()?.clone())
    }
    fn lock_cached_view(&self) -> Result<std::sync::MutexGuard<'_, Option<ViewResult>>> {
        self.cached_view
            .lock()
            .map_err(|error| anyhow::anyhow!("command view mutex poisoned: {error}"))
    }
}
fn failure_view(record: &CommandRecord, message: &str) -> Result<ViewResult> {
    let view = read_command_result(record, &record.initial_cwd)?;
    match view {
        ViewResult::Command {
            cwd,
            finished: false,
            stdout,
            mut stderr,
            ..
        } => {
            if !stderr.is_empty() {
                stderr.push('\n');
            }
            stderr.push_str(message);
            Ok(ViewResult::Command {
                cwd,
                finished: true,
                stdout,
                stderr,
                exit_code: Some(1_i32),
            })
        }
        other @ (ViewResult::Command { .. } | ViewResult::Tab { .. }) => Ok(other),
    }
}
impl ShellReservation {
    pub(super) fn new(
        session: &Arc<super::super::session::ShellSession>,
        command_id: &str,
    ) -> Result<Self> {
        session.reserve(command_id)?;
        Ok(Self {
            session: Arc::clone(session),
            command_id: command_id.to_owned(),
            released: false,
        })
    }
    pub(super) fn release(&mut self) -> Result<()> {
        if self.released {
            return Ok(());
        }
        self.session.release(&self.command_id)?;
        self.released = true;
        Ok(())
    }
}
impl Drop for ShellReservation {
    fn drop(&mut self) {
        if let Err(error) = self.release() {
            eprintln!("{error:#}");
        }
    }
}
