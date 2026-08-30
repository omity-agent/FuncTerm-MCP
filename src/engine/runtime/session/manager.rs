mod command;
mod launcher;
mod process;
mod process_tree;
mod shell_session;
mod tab;
use crate::runtime::config::Settings;
use crate::runtime::protocol::EnvironmentSnapshot;
use crate::shell::ShellChoice;
use anyhow::{Result, bail};
use std::path::Path;
pub(crate) struct Manager {
    launcher: launcher::ShellLauncher,
    tabs: tab::TabDirectory,
}
impl Manager {
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
        let tab_id = self.tabs.next_tab_id()?;
        let session =
            self.launcher
                .launch(&tab_id, starting_directory, starting_shell, environment)?;
        self.tabs.insert(tab::Tab::new(tab_id.clone(), session)?)?;
        Ok(tab_id)
    }
}
#[cfg(test)]
mod tests {
    use super::Manager;
    use crate::runtime::config::Settings;
    use crate::shell::ShellChoice;
    use std::path::Path;
    fn test_settings() -> Settings {
        Settings {
            daemon_service_name: format!("functerm/test/manager/{}", nanoid::nanoid!()),
            terminal_rows: 30,
            terminal_cols: 120,
            terminal_model_title: "FuncTerm".to_owned(),
            shell_startup_timeout_seconds: 10.0,
            powershell: vec!["powershell.exe".to_owned()],
            bash: "bash.exe".to_owned(),
            nushell: "nu.exe".to_owned(),
            zsh: "zsh".to_owned(),
            cmd: "cmd.exe".to_owned(),
            bun: "bun".to_owned(),
            python: vec!["python".to_owned()],
        }
    }
    #[test]
    fn missing_starting_directory_is_rejected_before_tab_creation() {
        let manager = Manager::new(test_settings()).unwrap();
        let error = manager
            .new_tab(
                Path::new("Z:\\definitely-missing-mcp-pty-cwd"),
                ShellChoice::PowerShell,
                &crate::runtime::protocol::EnvironmentSnapshot::capture(),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("starting_directory does not exist or is not a directory")
        );
    }
    #[test]
    fn immediately_exiting_shell_is_rejected_before_registration() {
        let mut settings = test_settings();
        settings.powershell = vec![immediately_exiting_executable().to_owned()];
        let manager = Manager::new(settings).unwrap();
        let error = manager
            .new_tab(
                crate::test_fs::temp_root().as_path(),
                ShellChoice::PowerShell,
                &crate::runtime::protocol::EnvironmentSnapshot::capture(),
            )
            .unwrap_err();
        assert!(error.to_string().contains("startup"), "{error:#}");
    }
    #[cfg(windows)]
    fn immediately_exiting_executable() -> &'static str {
        "whoami.exe"
    }
    #[cfg(not(windows))]
    fn immediately_exiting_executable() -> &'static str {
        "false"
    }
}
