mod commands;
mod lifecycle;
mod process_tree;
mod session;
mod startup;
mod state;
mod tabs;
#[cfg(test)]
mod tests;
mod views;
use crate::runtime::config::Settings;
use crate::runtime::session::records::CommandRecord;
use crate::shell::ShellChoice;
use alloc::sync::Arc;
use anyhow::{Result, bail};
use session::ShellSession;
use std::path::Path;
pub(crate) struct Manager {
    launcher: startup::ShellLauncher,
    tabs: tabs::TabRegistry,
    commands: commands::CommandRegistry,
}
impl Manager {
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        Ok(Self {
            launcher: startup::ShellLauncher::new(settings)?,
            tabs: tabs::TabRegistry::default(),
            commands: commands::CommandRegistry::default(),
        })
    }
    pub(crate) fn new_tab(
        &self,
        starting_directory: &Path,
        starting_shell: ShellChoice,
    ) -> Result<String> {
        if !starting_directory.is_dir() {
            bail!(
                "starting_directory does not exist or is not a directory: {}",
                starting_directory.display()
            );
        }
        let tab_id = self.next_tab_id()?;
        let session = self
            .launcher
            .launch(&tab_id, starting_directory, starting_shell)?;
        self.remember_tab(&tab_id, &session)?;
        self.tabs.insert_shell(tab_id.clone(), session)?;
        Ok(tab_id)
    }
    fn find_shell(&self, id: &str) -> Result<Option<Arc<ShellSession>>> {
        self.tabs.find_shell(id)
    }
    fn find_command(&self, id: &str) -> Result<Option<CommandRecord>> {
        self.commands.find(id)
    }
}
