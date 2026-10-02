use super::super::{shell_session::ShellSession, tab::Tab};
use super::{CommandWait, ManagedCommand};
use crate::runtime::session::records::remove_record_directory;
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
pub(super) struct ShellReservation {
    session: Arc<ShellSession>,
    command_id: String,
}
impl ShellReservation {
    pub(super) fn new(
        session: &Arc<ShellSession>,
        tab_id: &str,
        command: Arc<ManagedCommand>,
    ) -> Result<Self> {
        let command_id = command.id().to_owned();
        session.reserve(tab_id, command)?;
        Ok(Self {
            session: Arc::clone(session),
            command_id,
        })
    }
}
impl Drop for ShellReservation {
    fn drop(&mut self) {
        self.session.release(&self.command_id);
    }
}
impl Tab {
    pub(super) fn supervise(
        self: &Arc<Self>,
        command: Arc<ManagedCommand>,
        reservation: ShellReservation,
    ) {
        let tab = Arc::clone(self);
        tokio::spawn(async move {
            let session = Arc::clone(&reservation.session);
            let result = async { let observed = tokio :: select ! { completion = command . wait (Duration :: MAX) => completion ?, exited = reservation . session . wait_for_exit () => { exited ?; command . wait (Duration :: ZERO) . await ? } } ; match observed { CommandWait :: Finished => tab . finish_done_command (& command) . await , CommandWait :: Running => { tab . fail_command (& command , "shell exited before command wrote done.json" . to_owned ()) . await ? ; let closed_tab = Arc :: clone (& tab) ; let stopped_session = Arc :: clone (& reservation . session) ; tab . blocking . run (move | | closed_tab . close_session (& stopped_session)) . await } CommandWait :: Failed => Ok (()) , } } . await ;
            if let Err(error) = result {
                tracing :: error ! (command_id = % command . id () , error = % error , "command supervision failed");
                if let Err(failure) = tab
                    .fail_command(&command, format!("command supervision failed: {error:#}"))
                    .await
                {
                    tracing :: error ! (command_id = % command . id () , error = % failure , "failed to finalize command");
                }
            }
            if let Err(error) = tab
                .blocking
                .run(move || {
                    drop(reservation);
                    Ok(())
                })
                .await
            {
                tracing :: error ! (command_id = % command . id () , error = % error , "failed to release shell reservation");
            }
            let command_id = command.id().to_owned();
            if let Err(error) = tab.cleanup_command_record(command, session).await {
                tracing :: error ! (% command_id , error = % error , "failed to clean command record");
            }
        });
    }
    async fn cleanup_command_record(
        &self,
        command: Arc<ManagedCommand>,
        session: Arc<ShellSession>,
    ) -> Result<()> {
        tracing :: debug ! (command_id = % command . id () , "waiting for command resources to be released");
        tokio::select! { released = command . wait_released () => released ?, exited = session . wait_for_exit () => exited ?, }
        self . blocking . run (move | | { let result = remove_record_directory (command . record ()) ; tracing :: debug ! (command_id = % command . id () , removed = result . is_ok () , "command record cleanup completed") ; drop (session) ; result }) . await
    }
    pub(in crate::engine::runtime::session::manager) async fn fail_command(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
        message: String,
    ) -> Result<()> {
        let managed = Arc::clone(command);
        self.blocking
            .run(move || managed.mark_failed(message))
            .await
    }
}
