use super::Tab;
use crate::runtime::protocol::{KeyboardInput, RequestRejected, ViewResult};
use crate::runtime::session::manager::command::ManagedCommand;
use alloc::sync::Arc;
use anyhow::{Result, bail};
use core::time::Duration;
use dashmap::DashMap;
const ID_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
#[derive(Default)]
pub(in crate::engine::runtime::session::manager) struct TabDirectory {
    tabs: DashMap<String, Arc<Tab>>,
    commands: DashMap<String, (Arc<Tab>, Arc<ManagedCommand>)>,
}
impl TabDirectory {
    pub(in crate::engine::runtime::session::manager) async fn close(
        &self,
        tab_id: &str,
    ) -> Result<()> {
        self.require_tab(tab_id)?.close().await
    }
    pub(in crate::engine::runtime::session::manager) fn insert(&self, tab: Arc<Tab>) {
        self.tabs.insert(tab.id().to_owned(), tab);
    }
    pub(in crate::engine::runtime::session::manager) async fn manual_write(
        &self,
        tab_id: &str,
        input: KeyboardInput,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        self.require_tab(tab_id)?
            .manual_write(input, wait_timeout)
            .await
    }
    pub(in crate::engine::runtime::session::manager) async fn run(
        self: &Arc<Self>,
        tab_id: &str,
        command: String,
        separate_output: bool,
        wait_timeout: Duration,
    ) -> Result<(String, ViewResult)> {
        let tab = self.require_tab(tab_id)?;
        let command_id = self.next_id("command-");
        let directory = Arc::clone(self);
        let managed = tab
            .with_operation(move |owner| {
                let started = owner.dispatch_command(command_id, &command, separate_output)?;
                directory.commands.insert(
                    started.id().to_owned(),
                    (Arc::clone(owner), Arc::clone(&started)),
                );
                Ok(started)
            })
            .await?;
        let view = tab.command_view(&managed, wait_timeout).await?;
        Ok((managed.id().to_owned(), view))
    }
    pub(in crate::engine::runtime::session::manager) async fn view(
        &self,
        id: &str,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        if let Some(tab) = self.tab(id) {
            return tab.view(wait_timeout).await;
        }
        let command = self.commands.get(id).map(|entry| entry.value().clone());
        if let Some((owner, managed)) = command {
            return owner.command_view(&managed, wait_timeout).await;
        }
        Err(RequestRejected::new(format!("unknown id {id}")).into())
    }
    pub(in crate::engine::runtime::session::manager) async fn close_all(&self) -> Result<()> {
        let tabs: Vec<_> = self
            .tabs
            .iter()
            .filter(|entry| entry.value().has_session())
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
    pub(in crate::engine::runtime::session::manager) fn next_tab_id(&self) -> String {
        self.next_id("tab-")
    }
    fn require_tab(&self, tab_id: &str) -> Result<Arc<Tab>> {
        self.tab(tab_id).ok_or_else(|| {
            RequestRejected::new(format!(
                "Unknown Tab ID \"{tab_id}\", The ID does not exist, or the Tab has been reclaimed."
            ))
            .into()
        })
    }
    fn tab(&self, id: &str) -> Option<Arc<Tab>> {
        self.tabs.get(id).map(|entry| Arc::clone(entry.value()))
    }
    fn next_id(&self, prefix: &str) -> String {
        loop {
            let id = format!("{prefix}{}", nanoid::nanoid!(12, &ID_ALPHABET));
            if !self.tabs.contains_key(&id) && !self.commands.contains_key(&id) {
                return id;
            }
        }
    }
}
