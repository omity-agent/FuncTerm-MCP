mod snapshot;
mod tab_view;
use self::snapshot::TabSnapshot;
use super::command::ManagedCommand;
use super::shell_session::ShellSession;
use crate::runtime::protocol::{KeyboardInput, ShellView, ViewResult};
use crate::runtime::session::terminal::lock_mutex;
use alloc::sync::Arc;
use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::Mutex;
#[derive(Default)]
pub(super) struct TabDirectory {
    tabs: Mutex<HashMap<String, Arc<Tab>>>,
    commands: Mutex<HashMap<String, Arc<Tab>>>,
}
pub(super) struct Tab {
    id: String,
    session: Mutex<Option<Arc<ShellSession>>>,
    snapshot: Mutex<TabSnapshot>,
    commands: Mutex<HashMap<String, Arc<ManagedCommand>>>,
}
const ID_LENGTH: usize = 12;
const ID_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
impl TabDirectory {
    pub(super) fn insert(&self, tab: Tab) -> Result<()> {
        lock_mutex(&self.tabs, "tab")?.insert(tab.id().to_owned(), Arc::new(tab));
        Ok(())
    }
    pub(super) fn manual_write(
        &self,
        tab_id: &str,
        input: KeyboardInput,
        waiting: core::time::Duration,
    ) -> Result<ViewResult> {
        self.require_tab(tab_id)?.manual_write(input, waiting)
    }
    pub(super) fn send_command(
        &self,
        tab_id: &str,
        command: &str,
        waiting: core::time::Duration,
    ) -> Result<(String, crate::runtime::protocol::EndReason, ViewResult)> {
        let command_id = self.next_command_id()?;
        let tab = self.require_tab(tab_id)?;
        let started = tab.start_command(command_id.clone(), command)?;
        lock_mutex(&self.commands, "command owner")?.insert(command_id, Arc::clone(&tab));
        started.wait(waiting)
    }
    pub(super) fn view(&self, id: &str, waiting: core::time::Duration) -> Result<ViewResult> {
        let matching_tab = lock_mutex(&self.tabs, "tab")?.get(id).cloned();
        if let Some(found_tab) = matching_tab {
            return found_tab.view(waiting);
        }
        let command_owner = lock_mutex(&self.commands, "command owner")?
            .get(id)
            .cloned();
        if let Some(owner_tab) = command_owner {
            return owner_tab.command_view(id, waiting);
        }
        bail!("unknown id {id}")
    }
    pub(super) fn next_tab_id(&self) -> Result<String> {
        self.next_id("tab-")
    }
    pub(super) fn next_command_id(&self) -> Result<String> {
        self.next_id("command-")
    }
    fn require_tab(&self, tab_id: &str) -> Result<Arc<Tab>> {
        let matching_tab = lock_mutex(&self.tabs, "tab")?.get(tab_id).cloned();
        if let Some(found_tab) = matching_tab {
            return Ok(found_tab);
        }
        bail!("unknown tab id {tab_id}")
    }
    fn next_id(&self, prefix: &str) -> Result<String> {
        loop {
            let id = format!("{prefix}{}", random_id_suffix());
            if !self.id_exists(&id)? {
                return Ok(id);
            }
        }
    }
    fn id_exists(&self, id: &str) -> Result<bool> {
        let tab_exists = lock_mutex(&self.tabs, "tab")?.contains_key(id);
        let command_exists = lock_mutex(&self.commands, "command owner")?.contains_key(id);
        Ok(tab_exists || command_exists)
    }
}
impl Tab {
    pub(super) fn new(id: String, session: Arc<ShellSession>) -> Result<Self> {
        let snapshot = TabSnapshot::from_session(&session)?;
        Ok(Self {
            id,
            session: Mutex::new(Some(session)),
            snapshot: Mutex::new(snapshot),
            commands: Mutex::new(HashMap::new()),
        })
    }
    pub(super) fn id(&self) -> &str {
        &self.id
    }
    pub(super) fn live_session(&self) -> Result<Arc<ShellSession>> {
        let session = lock_mutex(&self.session, "tab session")?.clone();
        session.map_or_else(
            || bail!("tab id {} was generated, but its shell is gone", self.id),
            Ok,
        )
    }
    pub(super) fn remember(&self, session: &ShellSession) -> Result<TabSnapshot> {
        let mut snapshot = TabSnapshot::from_session(session)?;
        let mut stored = lock_mutex(&self.snapshot, "tab snapshot")?;
        if snapshot.screen.is_empty() && !stored.screen.is_empty() {
            snapshot.screen.clone_from(&stored.screen);
        }
        *stored = snapshot.clone();
        drop(stored);
        Ok(snapshot)
    }
    pub(super) fn close_session(&self, session: &ShellSession) -> Result<()> {
        self.remember(session)?;
        *lock_mutex(&self.session, "tab session")? = None;
        Ok(())
    }
    pub(super) fn snapshot_view(&self) -> Result<ViewResult> {
        Ok(lock_mutex(&self.snapshot, "tab snapshot")?
            .clone()
            .into_view(false))
    }
    pub(super) fn snapshot_shell_view(&self) -> Result<ShellView> {
        Ok(lock_mutex(&self.snapshot, "tab snapshot")?
            .clone()
            .shell_view(false))
    }
    pub(super) fn optional_session(&self) -> Result<Option<Arc<ShellSession>>> {
        Ok(lock_mutex(&self.session, "tab session")?.clone())
    }
    pub(super) fn insert_command(&self, command: Arc<ManagedCommand>) -> Result<()> {
        lock_mutex(&self.commands, "tab command")?.insert(command.id().to_owned(), command);
        Ok(())
    }
    pub(super) fn remove_command(&self, command_id: &str) -> Result<Option<Arc<ManagedCommand>>> {
        Ok(lock_mutex(&self.commands, "tab command")?.remove(command_id))
    }
    pub(super) fn find_command(&self, command_id: &str) -> Result<Option<Arc<ManagedCommand>>> {
        Ok(lock_mutex(&self.commands, "tab command")?
            .get(command_id)
            .cloned())
    }
}
fn random_id_suffix() -> String {
    nanoid::nanoid!(ID_LENGTH, &ID_ALPHABET)
}
