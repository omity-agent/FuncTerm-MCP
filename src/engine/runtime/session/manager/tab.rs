mod shutdown;
mod snapshot;
mod tab_view;
use self::snapshot::TabSnapshot;
use super::BlockingExecutor;
use super::command::ManagedCommand;
use super::shell_session::ShellSession;
use crate::runtime::protocol::{KeyboardInput, ShellView, ViewResult};
use alloc::sync::Arc;
use anyhow::{Result, bail};
use dashmap::DashMap;
use parking_lot::Mutex;
#[derive(Default)]
pub(super) struct TabDirectory {
    tabs: DashMap<String, Arc<Tab>>,
    commands: DashMap<String, Arc<Tab>>,
}
pub(super) struct Tab {
    id: String,
    pub(super) operation: tokio::sync::Mutex<()>,
    pub(super) blocking: BlockingExecutor,
    state: Mutex<TabState>,
    commands: DashMap<String, Arc<ManagedCommand>>,
}
struct TabState {
    session: Option<Arc<ShellSession>>,
    snapshot: TabSnapshot,
}
const ID_LENGTH: usize = 12;
const ID_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
impl TabDirectory {
    pub(super) async fn close(&self, tab_id: &str) -> Result<()> {
        self.require_tab(tab_id)?.close().await
    }
    pub(super) fn insert(&self, tab: Tab) {
        self.tabs.insert(tab.id().to_owned(), Arc::new(tab));
    }
    pub(super) async fn manual_write(
        &self,
        tab_id: &str,
        input: KeyboardInput,
        wait_timeout: core::time::Duration,
    ) -> Result<ViewResult> {
        self.require_tab(tab_id)?
            .manual_write(input, wait_timeout)
            .await
    }
    pub(super) async fn send_command(
        &self,
        tab_id: &str,
        command: &str,
        wait_timeout: core::time::Duration,
    ) -> Result<(String, crate::runtime::protocol::EndReason, ViewResult)> {
        let command_id = self.next_command_id();
        let tab = self.require_tab(tab_id)?;
        let started = tab
            .start_command(command_id.clone(), command.to_owned())
            .await?;
        self.commands.insert(command_id, Arc::clone(&tab));
        started.wait(wait_timeout).await
    }
    pub(super) async fn view(
        &self,
        id: &str,
        wait_timeout: core::time::Duration,
    ) -> Result<ViewResult> {
        let matching_tab = self.tabs.get(id).map(|entry| Arc::clone(entry.value()));
        if let Some(found_tab) = matching_tab {
            return found_tab.view(wait_timeout).await;
        }
        let command_owner = self.commands.get(id).map(|entry| Arc::clone(entry.value()));
        if let Some(owner_tab) = command_owner {
            return owner_tab.command_view(id, wait_timeout).await;
        }
        bail!("unknown id {id}")
    }
    pub(super) fn next_tab_id(&self) -> String {
        self.next_id("tab-")
    }
    pub(super) fn next_command_id(&self) -> String {
        self.next_id("command-")
    }
    fn require_tab(&self, tab_id: &str) -> Result<Arc<Tab>> {
        let matching_tab = self.tabs.get(tab_id).map(|entry| Arc::clone(entry.value()));
        if let Some(found_tab) = matching_tab {
            return Ok(found_tab);
        }
        bail!("unknown tab id {tab_id}")
    }
    fn next_id(&self, prefix: &str) -> String {
        loop {
            let id = format!("{prefix}{}", random_id_suffix());
            if !self.id_exists(&id) {
                return id;
            }
        }
    }
    fn id_exists(&self, id: &str) -> bool {
        self.tabs.contains_key(id) || self.commands.contains_key(id)
    }
}
impl Tab {
    pub(super) fn new(
        id: String,
        session: Arc<ShellSession>,
        blocking: BlockingExecutor,
    ) -> Result<Self> {
        let snapshot = TabSnapshot::from_session(&session)?;
        Ok(Self {
            id,
            operation: tokio::sync::Mutex::new(()),
            blocking,
            state: Mutex::new(TabState {
                session: Some(session),
                snapshot,
            }),
            commands: DashMap::new(),
        })
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    pub(super) fn live_session(&self) -> Result<Arc<ShellSession>> {
        let session = self.state.lock().session.clone();
        session.map_or_else(
            || bail!("tab id {} was generated, but its shell is gone", self.id),
            Ok,
        )
    }
    pub(super) fn remember(&self, session: &ShellSession) -> Result<TabSnapshot> {
        Ok(self.store_snapshot(TabSnapshot::from_session(session)?, false))
    }
    pub(super) fn close_session(&self, session: &ShellSession) -> Result<()> {
        self.store_snapshot(TabSnapshot::from_session(session)?, true);
        Ok(())
    }
    pub(super) fn snapshot_view(&self) -> ViewResult {
        self.state.lock().snapshot.clone().into_view(false)
    }
    pub(super) fn snapshot_shell_view(&self) -> ShellView {
        self.state.lock().snapshot.clone().shell_view(false)
    }
    pub(super) fn optional_session(&self) -> Option<Arc<ShellSession>> {
        self.state.lock().session.clone()
    }
    pub(super) fn insert_command(&self, command: Arc<ManagedCommand>) {
        self.commands.insert(command.id().to_owned(), command);
    }
    pub(super) fn remove_command(&self, command_id: &str) -> Option<Arc<ManagedCommand>> {
        self.commands
            .remove(command_id)
            .map(|(_id, command)| command)
    }
    pub(super) fn find_command(&self, command_id: &str) -> Option<Arc<ManagedCommand>> {
        self.commands
            .get(command_id)
            .map(|entry| Arc::clone(entry.value()))
    }
    fn store_snapshot(&self, mut snapshot: TabSnapshot, close: bool) -> TabSnapshot {
        let mut state = self.state.lock();
        if snapshot.screen.is_empty() && !state.snapshot.screen.is_empty() {
            snapshot.screen.clone_from(&state.snapshot.screen);
        }
        state.snapshot = snapshot.clone();
        if close {
            state.session = None;
        }
        snapshot
    }
}
fn random_id_suffix() -> String {
    nanoid::nanoid!(ID_LENGTH, &ID_ALPHABET)
}
