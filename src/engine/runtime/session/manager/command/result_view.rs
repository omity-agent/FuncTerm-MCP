use super::super::{
    shell_session::{KeyboardWriteFailure, ShellSession},
    tab::Tab,
};
use super::{CommandWait, ManagedCommand};
use crate::runtime::protocol::{CommandSnapshot, KeyboardInput, ViewResult};
use anyhow::{Context as _, Result};
use core::time::Duration;
impl Tab {
    pub(in crate::engine::runtime::session::manager) fn manual_write(
        &self,
        input: KeyboardInput,
        waiting: Duration,
    ) -> Result<ViewResult> {
        let session = self.live_session()?;
        if !session.is_alive()? {
            self.close_session(&session)?;
            anyhow::bail!("tab id {} was generated, but its shell is gone", self.id());
        }
        session.refresh_choice()?;
        match session.write_keyboard_for_running_command(input, waiting) {
            Ok(()) => {
                if session.is_alive()? {
                    Ok(self.remember(&session)?.into_view(true))
                } else {
                    self.close_session(&session)?;
                    self.snapshot_view()
                }
            }
            Err(KeyboardWriteFailure::IdlePrompt) => {
                anyhow::bail!(
                    "manual_write is unavailable while the prompt is idle; use send_command for prompt commands"
                )
            }
            Err(KeyboardWriteFailure::Write(error)) => {
                self.close_session(&session)?;
                Err(error)
            }
        }
    }
    pub(in crate::engine::runtime::session::manager) fn command_view(
        &self,
        command_id: &str,
        waiting: Duration,
    ) -> Result<ViewResult> {
        let command = self
            .find_command(command_id)?
            .with_context(|| format!("command owner is missing record: {command_id}"))?;
        match command.wait(waiting)? {
            CommandWait::Finished => self.finish_done_command(&command)?,
            CommandWait::Running => {
                if let Some(session) = self.optional_session()? {
                    self.abort_if_shell_dead(&session, &command)?;
                }
            }
            CommandWait::Failed => {}
        }
        self.command_view_result(&command)
    }
    pub(in crate::engine::runtime::session::manager) fn finish_done_command(
        &self,
        command: &ManagedCommand,
    ) -> Result<()> {
        let session = self.optional_session()?;
        if let Some(active_session) = session.as_ref() {
            active_session.update_cwd_from_done(command.record())?;
        }
        command.mark_finished()?;
        if let Some(active_session) = session {
            active_session.release(command.id())?;
            self.remember(&active_session)?;
        }
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) fn abort_if_shell_dead(
        &self,
        session: &ShellSession,
        command: &ManagedCommand,
    ) -> Result<bool> {
        if session.is_alive()? {
            return Ok(false);
        }
        command.mark_failed("shell exited before command wrote done.json")?;
        session.release(command.id())?;
        self.close_session(session)?;
        Ok(true)
    }
    pub(super) fn command_view_result(&self, command: &ManagedCommand) -> Result<ViewResult> {
        let snapshot = command.view()?;
        self.command_snapshot_result(snapshot)
    }
    fn command_snapshot_result(&self, snapshot: CommandSnapshot) -> Result<ViewResult> {
        let mut shell = if let Some(session) = self.optional_session()? {
            let alive = session.is_alive()?;
            if alive {
                self.remember(&session)?.shell_view(true)
            } else {
                self.snapshot_shell_view()?
            }
        } else {
            self.snapshot_shell_view()?
        };
        shell.title = snapshot.title;
        Ok(ViewResult::Command {
            shell,
            command: snapshot.command,
            note: snapshot.note,
        })
    }
}
