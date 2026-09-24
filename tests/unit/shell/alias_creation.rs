use super::{ensure_directory, environment};
use crate::runtime::config::Settings;
use crate::shell::ShellChoice;
#[test]
fn shim_aliases_share_storage_and_accept_repeated_creation() {
    let root = crate::test_fs::temp_dir("alias-storage");
    let executable = root.join("source");
    let first = root.join("bash");
    let second = root.join("python");
    std::fs::write(&executable, b"shared executable").unwrap();
    for alias in [&first, &second, &first] {
        super::create_shim_alias(&executable, alias, "test").unwrap();
    }
    assert!(same_file::is_same_file(&first, &second).unwrap());
    #[cfg(unix)]
    assert!(std::fs::symlink_metadata(&first).unwrap().is_symlink());
    std::fs::remove_dir_all(root).unwrap();
}
pub(super) fn test_settings() -> Settings {
    Settings {
        daemon_service_name: "functerm/test".to_owned(),
        terminal_rows: 30,
        terminal_cols: 120,
        terminal_model_title: "FuncTerm".to_owned(),
        shell_startup_timeout_seconds: 10.0,
        powershell: vec!["definitely-missing-powershell".to_owned()],
        bash: vec!["definitely-missing-bash".to_owned()],
        nushell: vec!["definitely-missing-nu".to_owned()],
        zsh: vec!["definitely-missing-zsh".to_owned()],
        cmd: vec!["definitely-missing-cmd".to_owned()],
        bun: vec!["definitely-missing-bun".to_owned()],
        python: vec!["definitely-missing-python".to_owned()],
        mcp: crate::runtime::config::McpSettings::default(),
    }
}
#[test]
fn environment_does_not_create_shim_directory() {
    let root = crate::test_fs::temp_dir("shim-environment");
    let session_root = root.join("session");
    let shim_dir = root.join("shims");
    let env = environment(
        &test_settings(),
        &session_root,
        &shim_dir,
        ShellChoice::PowerShell,
        &crate::runtime::protocol::EnvironmentSnapshot::capture(),
        &root,
    )
    .unwrap();
    assert!(!shim_dir.exists());
    assert!(env.iter().any(|item| item.0 == "FUNCTERM_SHIM_DIR"));
}
#[test]
fn ensure_directory_creates_shell_aliases() {
    let shim_dir = crate::test_fs::temp_dir("shim-aliases");
    ensure_directory(&shim_dir).unwrap();
    for &shell in ShellChoice::all() {
        for alias in shell.shim_executable_names() {
            assert!(shim_dir.join(alias).exists());
        }
    }
    std::fs::remove_dir_all(shim_dir).unwrap();
}
