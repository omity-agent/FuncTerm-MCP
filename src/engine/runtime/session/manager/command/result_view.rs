use super::super::tab::Tab;
use super::{CommandWait, ManagedCommand};
use crate::runtime::protocol::{CommandSnapshot, ViewResult};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
impl Tab {
    pub(in crate::engine::runtime::session::manager) async fn command_view(
        self: &Arc<Self>,
        command_id: &str,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        let command = self
            .find_command(command_id)
            .with_context(|| format!("command owner is missing record: {command_id}"))?;
        match self.observe_command(&command, wait_timeout).await? {
            CommandWait::Finished => self.finish_done_command(&command).await?,
            CommandWait::Running => {
                if let Some(session) = self.optional_session() {
                    self.abort_if_shell_dead(&session, &command).await?;
                }
            }
            CommandWait::Failed => {}
        }
        self.command_view_result(&command).await
    }
    pub(in crate::engine::runtime::session::manager) async fn observe_command(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
        wait_timeout: Duration,
    ) -> Result<CommandWait> {
        let observed = command.wait(wait_timeout).await;
        if !matches!(observed, Ok(CommandWait::Finished | CommandWait::Failed))
            && let Some(session) = self.optional_session()
            && self.abort_if_shell_dead(&session, command).await?
        {
            return Ok(CommandWait::Finished);
        }
        observed
    }
    pub(super) async fn command_view_result(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
    ) -> Result<ViewResult> {
        let _operation = self.operation.lock().await;
        let target = Arc::clone(self);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || target.command_snapshot_result(managed.view()?))
            .await
    }
    fn command_snapshot_result(&self, snapshot: CommandSnapshot) -> Result<ViewResult> {
        let mut shell = if let Some(session) = self.optional_session() {
            let alive = session.is_alive()?;
            self.remember(&session)?.shell_view(alive)
        } else {
            self.snapshot_shell_view()
        };
        shell.title = snapshot.title;
        Ok(ViewResult::Command {
            shell,
            command: snapshot.command,
            note: snapshot.note,
        })
    }
}
