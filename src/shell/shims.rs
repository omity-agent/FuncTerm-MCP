use super::ShellChoice;
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result};
use std::ffi::OsString;
use std::fs;
use std::path::Path;
const SHIM_DIR: &str = "shell-shims";
pub(crate) const ACTIVE_SHELL_FILE_ENV: &str = "FUNCTERM_ACTIVE_SHELL_FILE";
pub(crate) const CURRENT_SHELL_ENV: &str = "FUNCTERM_CURRENT_SHELL";
pub(crate) const SESSION_ROOT_ENV: &str = "FUNCTERM_SESSION_ROOT";
pub(crate) const SHIM_DIR_ENV: &str = "FUNCTERM_SHIM_DIR";
pub(crate) use crate::contract::{COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV};
pub(crate) fn environment(
    settings: &Settings,
    session_root: &Path,
    current_shell: ShellChoice,
) -> Result<Vec<(String, String)>> {
    let shim_dir = session_root.join(SHIM_DIR);
    fs::create_dir_all(&shim_dir).context("failed to create shell shim directory")?;
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    for shell in ShellChoice::all() {
        for alias in shell.executable_aliases() {
            fs::copy(&current_exe, shim_dir.join(alias))
                .with_context(|| format!("failed to create shell shim {alias}"))?;
        }
    }
    let mut env = vec![
        (
            "PATH".to_owned(),
            prepend_path(&shim_dir).context("failed to build shim PATH")?,
        ),
        (
            SHIM_DIR_ENV.to_owned(),
            path_text(&shim_dir).context("failed to encode shell shim directory")?,
        ),
        (
            SESSION_ROOT_ENV.to_owned(),
            path_text(session_root).context("failed to encode shell session root")?,
        ),
        (
            ACTIVE_SHELL_FILE_ENV.to_owned(),
            path_text(&session_root.join("active-shell.txt"))
                .context("failed to encode active shell state path")?,
        ),
        (
            CURRENT_SHELL_ENV.to_owned(),
            current_shell.canonical_name().to_owned(),
        ),
    ];
    for shell in ShellChoice::all() {
        if let Ok(executable) = shell.executable_path(settings) {
            env.push((
                shell.shim_env_name().to_owned(),
                path_text(&executable).with_context(|| {
                    format!(
                        "failed to encode {} executable path",
                        shell.canonical_name()
                    )
                })?,
            ));
        }
    }
    Ok(env)
}
pub(crate) fn write_active_shell(path: &Path, shell: ShellChoice) -> Result<()> {
    fs::write(path, shell.canonical_name())
        .with_context(|| format!("failed to write active shell state {}", path.display()))
}
pub(crate) fn read_active_shell(path: &Path) -> Result<Option<ShellChoice>> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read active shell state {}", path.display()))?;
    Ok(Some(ShellChoice::parse(text.trim())?))
}
fn prepend_path(shim_dir: &Path) -> Result<String> {
    let mut parts = vec![shim_dir.as_os_str().to_owned()];
    if let Some(path) = std::env::var_os("PATH") {
        parts.extend(std::env::split_paths(&path).map(std::path::PathBuf::into_os_string));
    }
    let joined = std::env::join_paths(parts).context("failed to join PATH entries")?;
    os_text(joined)
}
fn path_text(path: &Path) -> Result<String> {
    os_text(path.as_os_str().to_owned())
}
fn os_text(value: OsString) -> Result<String> {
    value
        .into_string()
        .map_err(|text| anyhow::anyhow!("value is not valid UTF-8: {}", text.to_string_lossy()))
}
