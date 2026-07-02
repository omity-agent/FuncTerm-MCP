use super::Manager;
use crate::runtime::config::Settings;
use crate::shell::ShellChoice;
use std::path::Path;
fn test_settings() -> Settings {
    Settings {
        daemon_service_name: "shell_mcp_pty/test".to_owned(),
        terminal_rows: 30,
        terminal_cols: 120,
        powershell: vec!["powershell.exe".to_owned()],
        bash: "bash.exe".to_owned(),
        nushell: "nu.exe".to_owned(),
        zsh: "zsh".to_owned(),
    }
}
#[test]
fn missing_starting_directory_is_rejected_before_tab_creation() {
    let manager = Manager::new(test_settings()).unwrap();
    let error = manager
        .new_tab(
            Path::new("Z:\\definitely-missing-mcp-pty-cwd"),
            ShellChoice::PowerShell,
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
        .new_tab(std::env::temp_dir().as_path(), ShellChoice::PowerShell)
        .unwrap_err();
    assert!(error.to_string().contains("startup"));
}
#[cfg(windows)]
fn immediately_exiting_executable() -> &'static str {
    "where.exe"
}
#[cfg(not(windows))]
fn immediately_exiting_executable() -> &'static str {
    "false"
}
#[test]
fn generated_ids_have_kind_prefixes_and_base36_suffixes() {
    let manager = Manager::new(test_settings()).unwrap();
    assert_id(&manager.tabs.next_tab_id().unwrap(), "tab-");
    assert_id(&manager.tabs.next_command_id().unwrap(), "command-");
}
fn assert_id(id: &str, prefix: &str) {
    let suffix = id.strip_prefix(prefix).unwrap();
    assert_eq!(suffix.len(), 12);
    assert!(
        suffix
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit())
    );
}
