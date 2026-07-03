mod lifecycle;
#[cfg(test)]
mod tests;
use super::{Manager, session::KeyboardWriteFailure, tab::Tab};
use crate::runtime::protocol::{CommandSnapshot, EndReason, ViewResult};
use crate::runtime::session::records::{create_record, remove_record_directory};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use lifecycle::ShellReservation;
pub(super) use lifecycle::{CommandWait, ManagedCommand};
use std::thread;
pub(super) struct StartedCommand {
    command: Arc<ManagedCommand>,
    tab: Arc<Tab>,
    session: Arc<super::session::ShellSession>,
}
impl Manager {
    pub(crate) fn manual_write(&self, tab_id: &str, bytes: &[u8]) -> Result<ViewResult> {
        self.tabs.manual_write(tab_id, bytes)
    }
    pub(crate) fn send_command(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        waiting: Duration,
    ) -> Result<(String, EndReason, ViewResult)> {
        self.tabs.send_command(tab_id, command, waiting)
    }
    pub(crate) fn view(&self, id: &str, waiting: Duration) -> Result<ViewResult> {
        self.tabs.view(id, waiting)
    }
}
impl Tab {
    pub(super) fn manual_write(&self, bytes: &[u8]) -> Result<ViewResult> {
        let session = self.live_session()?;
        if !session.is_alive()? {
            self.close_session(&session)?;
            anyhow::bail!("tab id {} was generated, but its shell is gone", self.id());
        }
        session.refresh_choice()?;
        match session.write_keyboard_for_running_command(bytes) {
            Ok(()) => Ok(self.remember(&session)?.into_view(true)),
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
    pub(super) fn start_command(
        self: &Arc<Self>,
        command_id: String,
        command_text: &str,
    ) -> Result<StartedCommand> {
        let session = self.live_session()?;
        if !session.is_alive()? {
            self.close_session(&session)?;
            anyhow::bail!("tab id {} was generated, but its shell is gone", self.id());
        }
        session.refresh_choice()?;
        self.remember(&session)?;
        let reservation = ShellReservation::new(&session, &command_id)?;
        let initial_cwd = session.cwd()?;
        let record = create_record(session.command_root(), &command_id, &initial_cwd)?;
        let managed = Arc::new(ManagedCommand::new(command_id, record));
        self.insert_command(Arc::clone(&managed))?;
        if let Err(error) = session.write_invocation(managed.id(), command_text, managed.record()) {
            self.remove_command(managed.id())?;
            if let Err(cleanup_error) = remove_record_directory(managed.record()) {
                eprintln!("{cleanup_error:#}");
            }
            self.close_session(&session)?;
            return Err(error);
        }
        if let Err(error) = session.wait_for_command_start(managed.record()) {
            self.remove_command(managed.id())?;
            if let Err(cleanup_error) = remove_record_directory(managed.record()) {
                eprintln!("{cleanup_error:#}");
            }
            self.close_session(&session)?;
            return Err(error);
        }
        self.start_monitor(Arc::clone(&managed), Arc::clone(&session), reservation);
        Ok(StartedCommand {
            command: managed,
            tab: Arc::clone(self),
            session,
        })
    }
    pub(super) fn command_view(&self, command_id: &str, waiting: Duration) -> Result<ViewResult> {
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
    fn start_monitor(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
        session: Arc<super::session::ShellSession>,
        mut reservation: ShellReservation,
    ) {
        let tab = Arc::clone(self);
        thread::spawn(move || {
            match command.wait(Duration::MAX) {
                Ok(CommandWait::Finished) => {
                    if let Err(error) = tab.finish_done_command(&command) {
                        eprintln!("{error:#}");
                    }
                }
                Ok(CommandWait::Running | CommandWait::Failed) => {}
                Err(error) => eprintln!("{error:#}"),
            }
            if let Err(error) = reservation.release() {
                eprintln!("{error:#}");
            }
            drop(session);
        });
    }
    pub(super) fn finish_done_command(&self, command: &ManagedCommand) -> Result<()> {
        if let Some(session) = self.optional_session()? {
            session.update_cwd_from_done(command.record())?;
            session.release(command.id())?;
            self.remember(&session)?;
        }
        command.mark_finished()?;
        Ok(())
    }
    pub(super) fn abort_if_shell_dead(
        &self,
        session: &super::session::ShellSession,
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
        let shell = if let Some(session) = self.optional_session()? {
            let alive = session.is_alive()?;
            if alive {
                self.remember(&session)?.shell_view(true)
            } else {
                self.snapshot_shell_view()?
            }
        } else {
            self.snapshot_shell_view()?
        };
        Ok(ViewResult::Command {
            shell,
            command: snapshot.command,
            note: snapshot.note,
        })
    }
}
impl StartedCommand {
    pub(super) fn wait(self, waiting: Duration) -> Result<(String, EndReason, ViewResult)> {
        let reason = match self.command.wait(waiting)? {
            CommandWait::Finished => {
                self.tab.finish_done_command(&self.command)?;
                EndReason::CommandEnded
            }
            CommandWait::Running => {
                if self.tab.abort_if_shell_dead(&self.session, &self.command)? {
                    EndReason::CommandFailed
                } else {
                    EndReason::WaitTimeout
                }
            }
            CommandWait::Failed => EndReason::CommandFailed,
        };
        let result = self.tab.command_view_result(&self.command)?;
        Ok((self.command.id().to_owned(), reason, result))
    }
}
