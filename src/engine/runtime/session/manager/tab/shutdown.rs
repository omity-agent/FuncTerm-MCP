use super::{Tab, TabDirectory};
use crate::runtime::session::manager::command::CommandWait;
use alloc::sync::Arc;
use anyhow::{Context as _, Result, bail};
use core::time::Duration;
impl TabDirectory {
    pub(in crate::engine::runtime::session::manager) async fn close_all(&self) -> Result<()> {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .map(|entry| Arc::clone(entry.value()))
            .collect();
        let results = futures_util::future::join_all(tabs.iter().map(Tab::close)).await;
        let failures: Vec<_> = results
            .into_iter()
            .zip(&tabs)
            .filter_map(|(result, tab)| {
                result.err().map(|error| format!("{}: {error:#}", tab.id()))
            })
            .collect();
        if !failures.is_empty() {
            bail!("failed to close daemon Tabs: {}", failures.join("; "));
        }
        Ok(())
    }
}
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
            active.freeze_output()?;
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
