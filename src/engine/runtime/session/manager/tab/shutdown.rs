use super::Tab;
use crate::runtime::session::manager::command::CommandWait;
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
impl Tab {
    pub(super) async fn close(self: &Arc<Self>) -> Result<()> {
        let _operation = self.operation.lock().await;
        let Some(session) = self.optional_session() else {
            return Ok(());
        };
        let command = if let Some(id) = session.busy_command_id() {
            Some(
                self.find_command(&id)
                    .with_context(|| format!("Tab is missing its running command {id}"))?,
            )
        } else {
            None
        };
        if let Some(active) = command.as_ref() {
            active.cancel_title_capture();
        }
        let target = Arc::clone(&session);
        self.blocking.run(move || target.terminate()).await?;
        if let Some(active) = command {
            match active.wait(Duration::ZERO).await? {
                CommandWait::Finished => self.finish_done_command(&active).await?,
                CommandWait::Running => {
                    self.fail_command(&active, "Tab was closed".to_owned())
                        .await?;
                }
                CommandWait::Failed => {}
            }
            session.release(active.id());
        }
        self.close_session(&session)
    }
}
