mod commands;
#[cfg(test)]
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
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use std::path::Path;
pub(crate) struct Manager {
    launcher: startup::ShellLauncher,
    tabs: tabs::TabDirectory,
}
impl Manager {
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        Ok(Self {
            launcher: startup::ShellLauncher::new(settings)?,
            tabs: tabs::TabDirectory::default(),
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
        let tab_id = self.tabs.next_tab_id()?;
        let session = self
            .launcher
            .launch(&tab_id, starting_directory, starting_shell)?;
        self.tabs.insert(tabs::Tab::new(tab_id.clone(), session)?)?;
        Ok(tab_id)
    }
}
