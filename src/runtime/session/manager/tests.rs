use super::Manager;
use crate::runtime::config::Settings;
use crate::shell::ShellChoice;
use std::path::Path;
fn test_settings() -> Settings {
    Settings {
        daemon_address: "127.0.0.1:43875".to_owned(),
        terminal_rows: 30,
        terminal_cols: 120,
        windows_powershell: "powershell.exe".to_owned(),
        pwsh: "pwsh.exe".to_owned(),
        bash: "bash.exe".to_owned(),
        nushell: "nu.exe".to_owned(),
    }
}
#[test]
fn missing_cwd_is_rejected_before_shell_creation() {
    let manager = Manager::new(test_settings()).unwrap();
    let error = manager
        .new_shell(
            Path::new("Z:\\definitely-missing-mcp-pty-cwd"),
            ShellChoice::PowerShell,
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("cwd does not exist or is not a directory")
    );
}
#[test]
fn immediately_exiting_shell_is_rejected_before_registration() {
    let mut settings = test_settings();
    settings.windows_powershell = "where.exe".to_owned();
    let manager = Manager::new(settings).unwrap();
    let error = manager
        .new_shell(std::env::temp_dir().as_path(), ShellChoice::PowerShell)
        .unwrap_err();
    assert!(error.to_string().contains("startup"));
}
#[test]
fn generated_ids_have_kind_prefixes_and_base36_suffixes() {
    let manager = Manager::new(test_settings()).unwrap();
    assert_id(&manager.next_shell_id().unwrap(), "shell-");
    assert_id(&manager.next_command_id().unwrap(), "command-");
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
