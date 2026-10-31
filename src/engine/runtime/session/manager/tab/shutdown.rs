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
    pub(in crate::engine::runtime::session::manager) async fn close(
        self: &Arc<Self>,
    ) -> Result<()> {
        tracing::debug!(tab_id = %self.id(), "waiting for tab operation before close");
        let _operation = self.operation.lock().await;
        tracing::debug!(tab_id = %self.id(), "closing tab session");
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
        let target = Arc::clone(&session);
        self.blocking.run(move || target.terminate()).await?;
        session.wait_for_terminal_drain().await?;
        let observed = match command.as_ref() {
            Some(active) => active.wait(Duration::ZERO).await?,
            None => CommandWait::Finished,
        };
        let tab = Arc::clone(self);
        self.blocking
            .run(move || {
                if let Some(active) = command {
                    match observed {
                        CommandWait::Finished => tab.cache_done_command(&active)?,
                        CommandWait::Running => active.mark_failed("Tab was closed")?,
                        CommandWait::Failed => {}
                    }
                    session.release(active.id());
                }
                tab.close_session(&session)
            })
            .await
    }
}
