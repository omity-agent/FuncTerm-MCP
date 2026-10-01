use super::super::{shell_session::ShellSession, tab::Tab};
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
        match command.wait(wait_timeout).await? {
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
    pub(in crate::engine::runtime::session::manager) async fn finish_done_command(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
    ) -> Result<()> {
        tracing :: debug ! (command_id = % command . id () , done = command . record () . done . try_exists () ?, "observed command completion; awaiting terminal title");
        let title = command.title.wait_finished().await?;
        let tab = Arc::clone(self);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || tab.cache_done_command(&managed, title))
            .await
    }
    fn cache_done_command(&self, command: &ManagedCommand, title: String) -> Result<()> {
        let session = self.optional_session();
        command.mark_finished(title, |cwd| {
            if let Some(active_session) = session.as_ref() {
                active_session.set_cwd(cwd);
            }
        })?;
        if let Some(active_session) = session {
            active_session.release(command.id());
            self.remember(&active_session)?;
        }
        Ok(())
    }
    pub(in crate::engine::runtime::session::manager) async fn abort_if_shell_dead(
        self: &Arc<Self>,
        session: &Arc<ShellSession>,
        command: &Arc<ManagedCommand>,
    ) -> Result<bool> {
        let target = Arc::clone(self);
        let active = Arc::clone(session);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || {
                if active.is_alive()? {
                    return Ok(false);
                }
                managed.mark_failed("shell exited before command wrote done.json")?;
                active.release(managed.id());
                target.close_session(&active)?;
                Ok(true)
            })
            .await
    }
    pub(super) async fn command_view_result(
        self: &Arc<Self>,
        command: &Arc<ManagedCommand>,
    ) -> Result<ViewResult> {
        let target = Arc::clone(self);
        let managed = Arc::clone(command);
        self.blocking
            .run(move || target.command_snapshot_result(managed.view()?))
            .await
    }
    fn command_snapshot_result(&self, snapshot: CommandSnapshot) -> Result<ViewResult> {
        let mut shell = if let Some(session) = self.optional_session() {
            let alive = session.is_alive()?;
            if alive {
                self.remember(&session)?.shell_view(true)
            } else {
                self.snapshot_shell_view()
            }
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
