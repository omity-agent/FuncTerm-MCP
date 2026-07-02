use super::{Manager, ShellSession};
use crate::runtime::protocol::ViewResult;
use crate::runtime::session::support::lock_mutex;
use alloc::sync::Arc;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Mutex;
#[derive(Default)]
pub(super) struct TabRegistry {
    shells: Mutex<HashMap<String, Arc<ShellSession>>>,
    snapshots: Mutex<HashMap<String, TabSnapshot>>,
}
#[derive(Clone)]
pub(super) struct TabSnapshot {
    cwd: String,
    screen: String,
    last_command: Option<String>,
}
impl TabRegistry {
    pub(super) fn insert_shell(&self, tab_id: String, shell: Arc<ShellSession>) -> Result<()> {
        lock_mutex(&self.shells, "shell")?.insert(tab_id, shell);
        Ok(())
    }
    pub(super) fn find_shell(&self, tab_id: &str) -> Result<Option<Arc<ShellSession>>> {
        Ok(lock_mutex(&self.shells, "shell")?.get(tab_id).cloned())
    }
    pub(super) fn remove_shell(&self, tab_id: &str) -> Result<Option<Arc<ShellSession>>> {
        Ok(lock_mutex(&self.shells, "shell")?.remove(tab_id))
    }
    pub(super) fn shell_id_exists(&self, tab_id: &str) -> Result<bool> {
        Ok(lock_mutex(&self.shells, "shell")?.contains_key(tab_id))
    }
    pub(super) fn remember(&self, tab_id: &str, shell: &ShellSession) -> Result<TabSnapshot> {
        let mut snapshot = TabSnapshot::from_shell(shell)?;
        let mut snapshots = lock_mutex(&self.snapshots, "tab snapshot")?;
        if snapshot.screen.is_empty()
            && let Some(previous) = snapshots.get(tab_id)
            && !previous.screen.is_empty()
        {
            snapshot.screen.clone_from(&previous.screen);
        }
        snapshots.insert(tab_id.to_owned(), snapshot.clone());
        drop(snapshots);
        Ok(snapshot)
    }
    pub(super) fn find_snapshot(&self, tab_id: &str) -> Result<Option<TabSnapshot>> {
        Ok(lock_mutex(&self.snapshots, "tab snapshot")?
            .get(tab_id)
            .cloned())
    }
    pub(super) fn generated_id_exists(&self, tab_id: &str) -> Result<bool> {
        Ok(lock_mutex(&self.snapshots, "tab snapshot")?.contains_key(tab_id))
    }
}
impl Manager {
    pub(super) fn remember_tab(&self, tab_id: &str, shell: &ShellSession) -> Result<TabSnapshot> {
        self.tabs.remember(tab_id, shell)
    }
    pub(super) fn find_tab_snapshot(&self, tab_id: &str) -> Result<Option<TabSnapshot>> {
        self.tabs.find_snapshot(tab_id)
    }
    pub(super) fn generated_tab_id_exists(&self, tab_id: &str) -> Result<bool> {
        self.tabs.generated_id_exists(tab_id)
    }
}
impl TabSnapshot {
    fn from_shell(shell: &ShellSession) -> Result<Self> {
        let cwd = super::state::path_text(&shell.cwd()?)?;
        let current_screen = lock_mutex(&shell.screen, "screen")?.screen().contents();
        let screen = if current_screen.is_empty() {
            cwd.clone()
        } else {
            current_screen
        };
        let last_command = lock_mutex(&shell.last_command, "last command")?.clone();
        Ok(Self {
            cwd,
            screen,
            last_command,
        })
    }
    pub(super) fn into_view(self, alive: bool) -> ViewResult {
        ViewResult::Tab {
            alive,
            cwd: self.cwd,
            screen: self.screen,
            last_command: self.last_command,
        }
    }
}
