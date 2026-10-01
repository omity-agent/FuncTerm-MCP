use super::lifecycle::{CommandWait, ManagedCommand};
use crate::runtime::protocol::CommandSnapshot;
use crate::runtime::session::records::{
    command_note, read_and_clear_command_result, read_command_result, read_done,
    remove_record_directory,
};
use anyhow::{Context as _, Result};
use std::path::PathBuf;
impl ManagedCommand {
    pub(super) fn view(&self) -> Result<CommandSnapshot> {
        let state = self.state.lock();
        let result = if let Some(view) = state.cached_view.as_ref() {
            Ok(view.clone())
        } else {
            read_command_result(&self.record, self.time_consumption(), self.title.current()?)
        };
        drop(state);
        result
    }
    pub(super) fn mark_finished(
        &self,
        title: String,
        update_cwd: impl FnOnce(PathBuf),
    ) -> Result<()> {
        let mut state = self.state.lock();
        if !matches!(state.wait, CommandWait::Running) {
            return Ok(());
        }
        let done =
            read_done(&self.record.done)?.context("completed command is missing done file")?;
        let mut view = read_and_clear_command_result(&self.record, self.time_consumption(), title)?;
        state.input.normalize(&mut view);
        update_cwd(PathBuf::from(done.cwd));
        state.cached_view = Some(view);
        state.wait = CommandWait::Finished;
        drop(state);
        self.wake();
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn mark_failed(
        &self,
        message: impl Into<String>,
    ) -> Result<()> {
        let failure_message = message.into();
        let mut state = self.state.lock();
        if !matches!(state.wait, CommandWait::Running) {
            return Ok(());
        }
        let title = self.title.cancel();
        let mut view = read_command_result(&self.record, self.time_consumption(), title)?;
        view.command.finished = true;
        view.command.exit_code = Some(1_i32);
        view.note = command_note(&view.command.stdout, &view.command.stderr, &failure_message);
        if let Err(error) = remove_record_directory(&self.record) {
            eprintln!("{error:#}");
        }
        state.cached_view = Some(view);
        state.wait = CommandWait::Failed;
        drop(state);
        self.wake();
        Ok(())
    }
}
