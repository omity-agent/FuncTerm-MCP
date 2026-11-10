use super::lifecycle::{CommandWait, ManagedCommand};
use crate::runtime::protocol::CommandSnapshot;
use crate::runtime::session::records::{command_note, read_done};
use anyhow::{Context as _, Result};
use std::path::PathBuf;
impl ManagedCommand {
    pub(in crate::engine::runtime::session::manager) fn mark_exited(
        &self,
        code: i32,
    ) -> Result<()> {
        let mut state = self.state.lock();
        if !matches!(state.wait, CommandWait::Running) {
            return Ok(());
        }
        let title = self.capture.cancel()?;
        let mut view = self.snapshot(
            &mut state,
            title,
            read_done(&self.record.done)?.as_ref(),
            true,
        )?;
        if !view.command.finished {
            view.command.finished = true;
            view.command.exit_code = Some(code);
        }
        state.input.normalize(&mut view);
        state.finish(view, CommandWait::Finished);
        drop(state);
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn publish_interruption(
        &self,
        cwd: &str,
    ) -> Result<()> {
        if self.record.done.try_exists()? {
            return Ok(());
        }
        anyhow::ensure!(
            self.state.lock().input.interrupted(),
            "shell returned to its prompt without publishing command completion"
        );
        let duration = format!("{}ns", self.time_consumption().as_nanos());
        let done = crate::contract::CommandCompletion {
            command_id: self.id(),
            exit_code: 130,
            time_consumption: duration.as_str(),
            cwd,
        };
        crate::publication::write_once(&self.record.done, sonic_rs::to_vec(&done)?)?;
        crate::publication::write_once(&self.record.released, b"")?;
        Ok(())
    }
    pub(super) fn view(&self) -> Result<CommandSnapshot> {
        let mut state = self.state.lock();
        let result = if let Some(view) = state.cached_view.as_ref() {
            Ok(view.clone())
        } else {
            self.snapshot(&mut state, self.capture.current()?, None, false)
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
        let mut view = self.snapshot(&mut state, title, Some(&done), true)?;
        state.input.normalize(&mut view);
        update_cwd(PathBuf::from(done.cwd));
        state.session_exit = self.record.session_exit.try_exists()?;
        state.finish(view, CommandWait::Finished);
        drop(state);
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
        let title = self.capture.cancel()?;
        let mut view = self.snapshot(
            &mut state,
            title,
            read_done(&self.record.done)?.as_ref(),
            true,
        )?;
        view.command.finished = true;
        view.command.exit_code = Some(1_i32);
        view.note = command_note(&view.command.output, &failure_message);
        state.finish(view, CommandWait::Failed);
        drop(state);
        Ok(())
    }
}
