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
        let observed = self.observe_command(&command, wait_timeout).await?;
        self.command_view_result(&command, observed).await
    }
    pub(in crate::engine::runtime::session::manager) async fn observe_command(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
        wait_timeout: Duration,
    ) -> Result<CommandWait> {
        let observed = match command.wait(wait_timeout).await {
            Ok(state) => state,
            Err(error) => {
                self.fail_command(command, format!("command observation failed: {error:#}"))
                    .await?;
                return Ok(CommandWait::Failed);
            }
        };
        if matches!(observed, CommandWait::Running)
            && let Some(session) = self.optional_session()
            && self.abort_if_shell_dead(&session, command).await?
        {
            return Ok(CommandWait::Finished);
        }
        Ok(observed)
    }
    pub(super) async fn command_view_result(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
        observed: CommandWait,
    ) -> Result<ViewResult> {
        if matches!(observed, CommandWait::Finished) {
            self.wait_done_capture(command).await?;
        }
        let _operation = self.operation.lock().await;
        let target = Arc::clone(self);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || {
                if matches!(observed, CommandWait::Finished) {
                    target.cache_done_command(&managed)?;
                }
                target.command_snapshot_result(managed.view()?)
            })
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
