use super::ShellChoice;
use crate::contract::HELPER_EXECUTABLE_ENV;
use crate::runtime::config::Settings;
use crate::runtime::protocol::EnvironmentSnapshot;
use anyhow::{Context as _, Result};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::Path;
pub(crate) const ACTIVE_SHELL_FILE_ENV: &str = "FUNCTERM_ACTIVE_SHELL_FILE";
pub(crate) const CURRENT_SHELL_ENV: &str = "FUNCTERM_CURRENT_SHELL";
pub(crate) const SESSION_ROOT_ENV: &str = "FUNCTERM_SESSION_ROOT";
pub(crate) const SHIM_DIR_ENV: &str = "FUNCTERM_SHIM_DIR";
pub(crate) use crate::contract::{COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV};
pub(crate) fn environment(
    settings: &Settings,
    session_root: &Path,
    shim_dir: &Path,
    current_shell: ShellChoice,
    inherited: &EnvironmentSnapshot,
    cwd: &Path,
) -> Result<Vec<(OsString, OsString)>> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let inherited_shim = inherited.value(SHIM_DIR_ENV);
    let path = prepend_path(shim_dir, inherited.value("PATH"), inherited_shim.as_deref())?;
    let mut env = vec![
        (OsString::from("PATH"), path),
        (
            OsString::from(SHIM_DIR_ENV),
            shim_dir.as_os_str().to_owned(),
        ),
        (
            OsString::from(SESSION_ROOT_ENV),
            session_root.as_os_str().to_owned(),
        ),
        (
            OsString::from(ACTIVE_SHELL_FILE_ENV),
            session_root
                .join("state")
                .join("active-shell.txt")
                .into_os_string(),
        ),
        (
            OsString::from(HELPER_EXECUTABLE_ENV),
            current_exe.into_os_string(),
        ),
        (
            OsString::from(CURRENT_SHELL_ENV),
            OsString::from(current_shell.canonical_name()),
        ),
    ];
    for shell in ShellChoice::all() {
        if let Ok(executable) = shell.executable_path(settings, inherited, cwd) {
            env.push((
                OsString::from(shell.shim_env_name()),
                executable.into_os_string(),
            ));
        }
    }
    let mut inherited_env = inherited
        .variables()
        .into_iter()
        .filter(|pair| !is_managed_name(&pair.0))
        .collect::<Vec<_>>();
    inherited_env.extend(env);
    Ok(inherited_env)
}
pub(crate) fn ensure_directory(shim_dir: &Path) -> Result<()> {
    fs::create_dir_all(shim_dir).context("failed to create shell shim directory")?;
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    for shell in ShellChoice::all() {
        for alias in shell.shim_executable_names() {
            create_shim_alias(&current_exe, &shim_dir.join(alias), alias)?;
        }
    }
    Ok(())
}
fn create_shim_alias(current_exe: &Path, alias_path: &Path, alias: &str) -> Result<()> {
    crate::file_publish::copy_once(current_exe, alias_path)
        .with_context(|| format!("failed to create shell shim {alias}"))
}
pub(crate) fn write_active_shell(path: &Path, shell: ShellChoice) -> Result<()> {
    crate::file_publish::write_replace(path, shell.canonical_name())
        .with_context(|| format!("failed to publish active shell state {}", path.display()))
}
pub(crate) fn read_active_shell(path: &Path) -> Result<Option<ShellChoice>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read active shell state {}", path.display()))?;
    Ok(Some(ShellChoice::from_canonical_name(text.trim())?))
}
fn prepend_path(
    shim_dir: &Path,
    inherited_path: Option<OsString>,
    inherited_shim: Option<&OsStr>,
) -> Result<OsString> {
    let mut parts = vec![shim_dir.as_os_str().to_owned()];
    if let Some(path) = inherited_path {
        parts.extend(
            std::env::split_paths(&path)
                .filter(|entry| !inherited_shim.is_some_and(|old| path_equals(entry, old)))
                .map(std::path::PathBuf::into_os_string),
        );
    }
    std::env::join_paths(parts).context("failed to join PATH entries")
}
fn is_managed_name(name: &OsStr) -> bool {
    let fixed = [
        "PATH",
        SHIM_DIR_ENV,
        SESSION_ROOT_ENV,
        ACTIVE_SHELL_FILE_ENV,
        HELPER_EXECUTABLE_ENV,
        CURRENT_SHELL_ENV,
        COMMAND_ID_ENV,
        COMMAND_DIRECTORY_ENV,
    ];
    fixed
        .iter()
        .any(|expected| environment_name_equals(name, expected))
        || ShellChoice::all()
            .iter()
            .any(|shell| environment_name_equals(name, shell.shim_env_name()))
}
#[cfg(windows)]
fn environment_name_equals(actual: &OsStr, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}
#[cfg(not(windows))]
fn environment_name_equals(actual: &OsStr, expected: &str) -> bool {
    actual == expected
}
#[cfg(windows)]
fn path_equals(actual: &Path, expected: &OsStr) -> bool {
    actual.as_os_str().eq_ignore_ascii_case(expected)
}
#[cfg(not(windows))]
fn path_equals(actual: &Path, expected: &OsStr) -> bool {
    actual.as_os_str() == expected
}
#[cfg(test)]
#[path = "shims/tests.rs"]
mod environment_tests;
#[cfg(test)]
mod tests {
    use super::{ensure_directory, environment};
    use crate::runtime::config::Settings;
    use crate::shell::ShellChoice;
    pub(super) fn test_settings() -> Settings {
        Settings {
            daemon_service_name: "functerm/test".to_owned(),
            terminal_rows: 30,
            terminal_cols: 120,
            terminal_initial_title: "FuncTerm".to_owned(),
            shell_startup_timeout_seconds: 10.0,
            powershell: vec!["definitely-missing-powershell".to_owned()],
            bash: "definitely-missing-bash".to_owned(),
            nushell: "definitely-missing-nu".to_owned(),
            zsh: "definitely-missing-zsh".to_owned(),
            cmd: "definitely-missing-cmd".to_owned(),
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
        for shell in ShellChoice::all() {
            for alias in shell.shim_executable_names() {
                assert!(shim_dir.join(alias).exists());
            }
        }
        std::fs::remove_dir_all(shim_dir).unwrap();
    }
}
