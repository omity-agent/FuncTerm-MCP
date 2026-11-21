use super::{Tab, TabDirectory};
use crate::runtime::session::manager::command::SessionEnd;
use alloc::sync::Arc;
use anyhow::{Result, bail};
impl TabDirectory {
    pub(in crate::engine::runtime::session::manager) async fn close_all(&self) -> Result<()> {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .filter(|entry| entry.value().optional_session().is_some())
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
        self.with_operation(|tab| {
            if let Some(session) = tab.optional_session() {
                tab.end_session(&session, SessionEnd::Closed)?;
            }
            Ok(())
        })
        .await
    }
}
