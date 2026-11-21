mod shutdown;
mod snapshot;
mod tab_view;
use self::snapshot::TabSnapshot;
use super::BlockingExecutor;
use super::command::ManagedCommand;
use super::shell_session::ShellSession;
use crate::runtime::protocol::{
    KeyboardInput, OutputTokenizer, RequestRejected, ShellView, ViewResult,
};
use alloc::sync::Arc;
use anyhow::Result;
use dashmap::DashMap;
use parking_lot::Mutex;
use tokio_util::task::TaskTracker;
#[derive(Default)]
pub(super) struct TabDirectory {
    tabs: DashMap<String, Arc<Tab>>,
    commands: DashMap<String, (Arc<Tab>, Arc<ManagedCommand>)>,
}
pub(super) struct Tab {
    id: String,
    operation: Arc<tokio::sync::Mutex<()>>,
    pub(super) blocking: BlockingExecutor,
    pub(super) tokenizer: Arc<OutputTokenizer>,
    pub(super) supervisors: TaskTracker,
    state: Mutex<TabState>,
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
    pub(super) async fn run(
        self: &Arc<Self>,
        tab_id: &str,
        command: &str,
        separate_output: bool,
        wait_timeout: core::time::Duration,
    ) -> Result<(String, ViewResult)> {
        let command_id = self.next_command_id();
        let tab = self.require_tab(tab_id)?;
        let directory = Arc::clone(self);
        let source = command.to_owned();
        let managed = tab
            .with_operation(move |owner| {
                let started = owner.dispatch_command(command_id, &source, separate_output)?;
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
    pub(super) async fn view(
        &self,
        id: &str,
        wait_timeout: core::time::Duration,
    ) -> Result<ViewResult> {
        let matching_tab = self.tabs.get(id).map(|entry| Arc::clone(entry.value()));
        if let Some(found_tab) = matching_tab {
            return found_tab.view(wait_timeout).await;
        }
        let matching_command = self.commands.get(id).map(|entry| entry.value().clone());
        if let Some((owner, command)) = matching_command {
            return owner.command_view(&command, wait_timeout).await;
        }
        Err(RequestRejected::new(format!("unknown id {id}")).into())
    }
    pub(super) fn next_tab_id(&self) -> String {
        self.next_id("tab-")
    }
    fn next_command_id(&self) -> String {
        self.next_id("command-")
    }
    fn require_tab(&self, tab_id: &str) -> Result<Arc<Tab>> {
        let matching_tab = self.tabs.get(tab_id).map(|entry| Arc::clone(entry.value()));
        if let Some(found_tab) = matching_tab {
            return Ok(found_tab);
        }
        Err(RequestRejected::new(format!(
            "Unknown Tab ID \"{tab_id}\", The ID does not exist, or the Tab has been reclaimed."
        ))
        .into())
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
        tokenizer: Arc<OutputTokenizer>,
        supervisors: TaskTracker,
    ) -> Result<Self> {
        let snapshot = TabSnapshot::from_session(&session)?;
        Ok(Self {
            id,
            operation: Arc::default(),
            blocking,
            tokenizer,
            supervisors,
            state: Mutex::new(TabState {
                session: Some(session),
                snapshot,
            }),
        })
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    pub(super) async fn with_operation<T>(
        self: &Arc<Self>,
        operation: impl FnOnce(&Arc<Self>) -> Result<T> + Send + 'static,
    ) -> Result<T>
    where
        T: Send + 'static,
    {
        let guard = Arc::clone(&self.operation).lock_owned().await;
        let tab = Arc::clone(self);
        self.blocking
            .run(move || {
                let _guard = guard;
                operation(&tab)
            })
            .await
    }
    pub(super) fn closed_tab(&self) -> RequestRejected {
        RequestRejected::new(format!(
            "\"{}\" exists in the records, but the tab has been closed. Did you run an exit command within it?",
            self.id
        ))
    }
    pub(super) fn live_session(&self) -> Result<Arc<ShellSession>> {
        Ok(self.optional_session().ok_or_else(|| self.closed_tab())?)
    }
    pub(super) fn remember(&self, session: &ShellSession) -> Result<TabSnapshot> {
        let snapshot = TabSnapshot::from_session(session)?;
        self.state.lock().snapshot = snapshot.clone();
        Ok(snapshot)
    }
    pub(super) fn record_closure(&self, session: &ShellSession) -> Result<()> {
        let snapshot = TabSnapshot::from_session(session);
        let mut state = self.state.lock();
        state.session = None;
        state.snapshot = snapshot?;
        drop(state);
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
}
fn random_id_suffix() -> String {
    nanoid::nanoid!(ID_LENGTH, &ID_ALPHABET)
}
