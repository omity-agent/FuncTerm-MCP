mod command;
mod launcher;
mod shell_session;
mod tab;
use crate::runtime::config::Settings;
use crate::runtime::protocol::EnvironmentSnapshot;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use shell_session::process;
use std::path::Path;
pub(crate) struct Manager {
    launcher: launcher::ShellLauncher,
    tabs: tab::TabDirectory,
}
impl Manager {
    pub(crate) fn close(&self, tab_id: &str) -> Result<()> {
        self.tabs.close(tab_id)
    }
    pub(crate) fn new(settings: Settings) -> Result<Self> {
        Ok(Self {
            launcher: launcher::ShellLauncher::new(settings)?,
            tabs: tab::TabDirectory::default(),
        })
    }
    pub(crate) fn new_tab(
        &self,
        starting_directory: &Path,
        starting_shell: ShellChoice,
        environment: &EnvironmentSnapshot,
    ) -> Result<String> {
        if !starting_directory.is_dir() {
            bail!(
                "starting_directory does not exist or is not a directory: {}",
                starting_directory.display()
            );
        }
        let tab_id = self.tabs.next_tab_id();
        let session =
            self.launcher
                .launch(&tab_id, starting_directory, starting_shell, environment)?;
        self.tabs.insert(tab::Tab::new(tab_id.clone(), session)?);
        Ok(tab_id)
    }
}
