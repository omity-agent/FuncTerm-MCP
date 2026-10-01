use super::Tab;
use crate::runtime::protocol::ViewResult;
use alloc::sync::Arc;
use anyhow::Result;
impl Tab {
    pub(super) async fn view(
        self: &Arc<Self>,
        wait_timeout: core::time::Duration,
    ) -> Result<ViewResult> {
        let Ok(session) = self.live_session() else {
            return Ok(self.snapshot_view());
        };
        let busy_command_id = session.busy_command_id();
        let Some(command_id) = busy_command_id else {
            let deadline = tokio::time::Instant::now()
                .checked_add(wait_timeout)
                .ok_or_else(|| anyhow::anyhow!("wait_timeout exceeds the supported timer range"))?;
            tokio::time::sleep_until(deadline).await;
            let idle_tab = Arc::clone(self);
            return self.blocking.run(move || idle_tab.tab_view(&session)).await;
        };
        if let Some(command) = self.find_command(&command_id) {
            match command.wait(wait_timeout).await? {
                super::super::command::CommandWait::Finished => {
                    self.finish_done_command(&command).await?;
                }
                super::super::command::CommandWait::Running => {
                    self.abort_if_shell_dead(&session, &command).await?;
                }
                super::super::command::CommandWait::Failed => {}
            }
        }
        let observed_tab = Arc::clone(self);
        self.blocking
            .run(move || observed_tab.tab_view(&session))
            .await
    }
    fn tab_view(&self, session: &super::super::shell_session::ShellSession) -> Result<ViewResult> {
        let alive = session.is_alive()?;
        session.refresh_choice()?;
        if alive {
            let snapshot = self.remember(session)?;
            Ok(snapshot.into_view(true))
        } else {
            if let Some(command_id) = session.busy_command_id()
                && let Some(command) = self.find_command(&command_id)
            {
                command.mark_failed("shell exited before command wrote done.json")?;
                session.release(command.id());
            }
            self.close_session(session)?;
            Ok(self.snapshot_view())
        }
    }
}
