use super::super::{shell_session::ShellSession, tab::Tab};
use super::supervision::ShellReservation;
use super::{CommandWait, ManagedCommand};
use crate::runtime::protocol::{EndReason, ViewResult};
use crate::runtime::session::records::{create_record, remove_record_directory};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
pub(in crate::engine::runtime::session::manager) struct StartedCommand {
    command: Arc<ManagedCommand>,
    tab: Arc<Tab>,
    session: Arc<ShellSession>,
}
impl Tab {
    pub(in crate::engine::runtime::session::manager) async fn start_command(
        self: &Arc<Self>,
        command_id: String,
        command_text: String,
    ) -> Result<StartedCommand> {
        let _operation = self.operation.lock().await;
        let tab = Arc::clone(self);
        let started = self
            .blocking
            .run(move || tab.dispatch_command(command_id, &command_text))
            .await?;
        if let Err(error) = started
            .session
            .wait_for_command_start(&started.command)
            .await
        {
            let failed_tab = Arc::clone(self);
            let command = Arc::clone(&started.command);
            let session = Arc::clone(&started.session);
            self.blocking
                .run(move || failed_tab.abandon_start(&command, &session))
                .await?;
            return Err(error);
        }
        Ok(started)
    }
    fn dispatch_command(
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
        let initial_cwd = session.cwd();
        let record = create_record(session.command_root(), &command_id, &initial_cwd)?;
        let title = session.capture_title(&command_id)?;
        let managed = Arc::new(ManagedCommand::new(
            command_id,
            record,
            Arc::clone(&title),
            session.command_watch(),
        ));
        let reservation = match ShellReservation::new(&session, self.id(), Arc::clone(&managed)) {
            Ok(reservation) => reservation,
            Err(error) => {
                drop(title.cancel());
                if let Err(remove_error) = remove_record_directory(managed.record()) {
                    eprintln!("{remove_error:#}");
                }
                return Err(error);
            }
        };
        self.insert_command(Arc::clone(&managed));
        if let Err(error) = session.write_invocation(managed.id(), command_text, managed.record()) {
            self.abandon_start(&managed, &session)?;
            return Err(error);
        }
        self.supervise(Arc::clone(&managed), reservation);
        Ok(StartedCommand {
            command: managed,
            tab: Arc::clone(self),
            session,
        })
    }
    fn abandon_start(&self, command: &ManagedCommand, session: &ShellSession) -> Result<()> {
        command.mark_failed("shell failed to start command")?;
        drop(self.remove_command(command.id()));
        if let Err(error) = remove_record_directory(command.record()) {
            eprintln!("{error:#}");
        }
        self.close_session(session)
    }
}
impl StartedCommand {
    pub(in crate::engine::runtime::session::manager) async fn wait(
        self,
        wait_timeout: Duration,
    ) -> Result<(String, EndReason, ViewResult)> {
        let reason = match self.command.wait(wait_timeout).await? {
            CommandWait::Finished => {
                self.tab.finish_done_command(&self.command).await?;
                EndReason::CommandEnded
            }
            CommandWait::Running => {
                if self
                    .tab
                    .abort_if_shell_dead(&self.session, &self.command)
                    .await?
                {
                    EndReason::CommandFailed
                } else {
                    tracing :: debug ! (command_id = % self . command . id () , started = self . command . record () . started . try_exists () ?, done = self . command . record () . done . try_exists () ?, shell_alive = self . session . is_alive () ?, "command wait timed out");
                    EndReason::WaitTimeout
                }
            }
            CommandWait::Failed => EndReason::CommandFailed,
        };
        let result = self.tab.command_view_result(&self.command).await?;
        Ok((self.command.id().to_owned(), reason, result))
    }
}
