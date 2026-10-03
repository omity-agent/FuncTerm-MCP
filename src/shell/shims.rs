use super::ShellChoice;
use crate::contract::HELPER_EXECUTABLE_ENV;
use crate::runtime::config::{DAEMON_SERVICE_NAME_ENV, Settings};
use crate::runtime::protocol::EnvironmentSnapshot;
use anyhow::{Context as _, Result};
use fs_err as fs;
use std::ffi::{OsStr, OsString};
use std::path::Path;
pub(crate) const ACTIVE_SHELL_FILE_ENV: &str = "FUNCTERM_ACTIVE_SHELL_FILE";
pub(crate) const CURRENT_SHELL_ENV: &str = "FUNCTERM_CURRENT_SHELL";
pub(crate) const LOAD_PROFILE_ENV: &str = "FUNCTERM_LOAD_PROFILE";
pub(crate) const PATH_ENV: &str = "PATH";
pub(crate) const SESSION_ROOT_ENV: &str = "FUNCTERM_SESSION_ROOT";
pub(crate) const SHIM_DIR_ENV: &str = "FUNCTERM_SHIM_DIR";
pub(crate) const TAB_ID_ENV: &str = "FUNCTERM_TAB_ID";
pub(crate) const PROTECTED_ENVIRONMENT_NAMES: [&str; 15] = [
    TAB_ID_ENV,
    DAEMON_SERVICE_NAME_ENV,
    SHIM_DIR_ENV,
    SESSION_ROOT_ENV,
    ACTIVE_SHELL_FILE_ENV,
    HELPER_EXECUTABLE_ENV,
    CURRENT_SHELL_ENV,
    LOAD_PROFILE_ENV,
    "FUNCTERM_REAL_POWERSHELL",
    "FUNCTERM_REAL_BASH",
    "FUNCTERM_REAL_NUSHELL",
    "FUNCTERM_REAL_ZSH",
    "FUNCTERM_REAL_CMD",
    "FUNCTERM_REAL_BUN",
    "FUNCTERM_REAL_PYTHON",
];
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
    let path = prepend_path(
        shim_dir,
        inherited.value(PATH_ENV),
        inherited_shim.as_deref(),
    )?;
    let mut env = vec![
        (OsString::from(PATH_ENV), path),
        (
            OsString::from(DAEMON_SERVICE_NAME_ENV),
            OsString::from(&settings.daemon_service_name),
        ),
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
    for &shell in ShellChoice::all() {
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
    fs::create_dir_all(shim_dir)?;
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    #[cfg(windows)]
    let alias_source = {
        let snapshot = shim_dir.join("dispatcher.exe");
        crate::publication::copy_once(&current_exe, &snapshot)?;
        snapshot
    };
    #[cfg(unix)]
    let alias_source = current_exe;
    for &shell in ShellChoice::all() {
        for alias in shell.shim_executable_names() {
            create_shim_alias(&alias_source, &shim_dir.join(alias), alias)?;
        }
    }
    Ok(())
}
fn create_shim_alias(current_exe: &Path, alias_path: &Path, alias: &str) -> Result<()> {
    #[cfg(unix)]
    let created = std::os::unix::fs::symlink(current_exe, alias_path);
    #[cfg(windows)]
    let created = std::fs::hard_link(current_exe, alias_path);
    match created {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            anyhow::ensure!(
                same_file::is_same_file(current_exe, alias_path)?,
                "shell shim {alias} does not reference the current executable"
            );
            Ok(())
        }
        Err(error) => Err(error).with_context(|| format!("failed to create shell shim {alias}")),
    }
}
pub(crate) fn write_active_shell(path: &Path, shell: ShellChoice) -> Result<()> {
    crate::publication::write_replace(path, shell.canonical_name())
        .with_context(|| format!("failed to publish active shell state {}", path.display()))
}
pub(crate) fn read_active_shell(path: &Path) -> Result<Option<ShellChoice>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(ShellChoice::from_canonical_name(text.trim())?)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
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
    environment_name_equals(name, PATH_ENV)
        || PROTECTED_ENVIRONMENT_NAMES
            .iter()
            .any(|expected| environment_name_equals(name, expected))
        || [COMMAND_ID_ENV, COMMAND_DIRECTORY_ENV]
            .iter()
            .any(|expected| environment_name_equals(name, expected))
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
