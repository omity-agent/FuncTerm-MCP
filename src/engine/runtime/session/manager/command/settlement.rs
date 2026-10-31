use super::super::{shell_session::ShellSession, tab::Tab};
use super::{CommandWait, ManagedCommand};
use alloc::sync::Arc;
use anyhow::Result;
impl Tab {
    pub(in crate::engine::runtime::session::manager) async fn finish_done_command(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
    ) -> Result<()> {
        if !matches!(command.state.lock().wait, CommandWait::Running) {
            return Ok(());
        }
        if let Some(session) = self.optional_session() {
            tokio::select! {
                captured = command.capture.wait_finished() => { captured?; }
                exited = session.wait_for_exit() => {
                    exited?;
                    self.abort_if_shell_dead(&session, command).await?;
                    return Ok(());
                }
            }
        }
        let _operation = self.operation.lock().await;
        let tab = Arc::clone(self);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || tab.cache_done_command(&managed))
            .await
    }
    pub(in crate::engine::runtime::session::manager) fn cache_done_command(
        &self,
        command: &ManagedCommand,
    ) -> Result<()> {
        if !matches!(command.state.lock().wait, CommandWait::Running) {
            return Ok(());
        }
        tracing::debug!(command_id = %command.id(), "publishing settled command");
        let session = self.optional_session();
        command.mark_finished(command.capture.current()?, |cwd| {
            if let Some(active) = session.as_ref() {
                active.set_cwd(cwd);
            }
        })?;
        if let Some(active) = session {
            active.release(command.id());
            if command.requests_session_exit()? {
                self.close_session(&active)?;
            } else {
                self.remember(&active)?;
            }
        }
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) async fn abort_if_shell_dead(
        self: &Arc<Self>,
        session: &Arc<ShellSession>,
        command: &Arc<ManagedCommand>,
    ) -> Result<bool> {
        let _operation = self.operation.lock().await;
        let exiting = Arc::clone(session);
        let exit_status = self
            .blocking
            .run(move || {
                let Some(code) = exiting.exit_code()? else {
                    return Ok(None);
                };
                exiting.terminate()?;
                Ok(Some(code))
            })
            .await?;
        let Some(code) = exit_status else {
            return Ok(false);
        };
        session.wait_for_terminal_drain().await?;
        let tab = Arc::clone(self);
        let active = Arc::clone(session);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || {
                managed.mark_exited(code)?;
                active.release(managed.id());
                tab.close_session(&active)?;
                Ok(true)
            })
            .await
    }
}
