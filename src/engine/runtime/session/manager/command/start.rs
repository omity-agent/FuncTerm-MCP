use super::super::{shell_session::ShellSession, tab::Tab};
use super::{CommandWait, ManagedCommand};
use crate::runtime::protocol::{EndReason, ViewResult};
use crate::runtime::session::records::{create_record, remove_record_directory};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
use std::thread;
pub(in crate::engine::runtime::session::manager) struct StartedCommand {
    command: Arc<ManagedCommand>,
    tab: Arc<Tab>,
    session: Arc<ShellSession>,
}
struct ShellReservation {
    session: Arc<ShellSession>,
    command_id: String,
    released: bool,
}
impl Tab {
    pub(in crate::engine::runtime::session::manager) fn start_command(
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
            self.abandon_start(&managed, &session)?;
            return Err(error);
        }
        if let Err(error) = session.wait_for_command_start(managed.record()) {
            self.abandon_start(&managed, &session)?;
            return Err(error);
        }
        self.start_monitor(Arc::clone(&managed), Arc::clone(&session), reservation);
        Ok(StartedCommand {
            command: managed,
            tab: Arc::clone(self),
            session,
        })
    }
    fn start_monitor(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
        session: Arc<ShellSession>,
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
    fn abandon_start(&self, command: &ManagedCommand, session: &ShellSession) -> Result<()> {
        self.remove_command(command.id())?;
        if let Err(error) = remove_record_directory(command.record()) {
            eprintln!("{error:#}");
        }
        self.close_session(session)
    }
}
impl StartedCommand {
    pub(in crate::engine::runtime::session::manager) fn wait(
        self,
        waiting: Duration,
    ) -> Result<(String, EndReason, ViewResult)> {
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
impl ShellReservation {
    fn new(session: &Arc<ShellSession>, command_id: &str) -> Result<Self> {
        session.reserve(command_id)?;
        Ok(Self {
            session: Arc::clone(session),
            command_id: command_id.to_owned(),
            released: false,
        })
    }
    fn release(&mut self) -> Result<()> {
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
