use super::ShellChoice;
use crate::contract::HELPER_EXECUTABLE_ENV;
use crate::runtime::config::Settings;
use anyhow::{Context as _, Result};
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
) -> Result<Vec<(String, String)>> {
    let current_exe = std::env::current_exe().context("failed to resolve current executable")?;
    let mut env = vec![
        (
            "PATH".to_owned(),
            prepend_path(shim_dir).context("failed to build shim PATH")?,
        ),
        (
            SHIM_DIR_ENV.to_owned(),
            crate::text::path_text(shim_dir, "shell shim directory")
                .context("failed to encode shell shim directory")?,
        ),
        (
            SESSION_ROOT_ENV.to_owned(),
            crate::text::path_text(session_root, "shell session root")
                .context("failed to encode shell session root")?,
        ),
        (
            ACTIVE_SHELL_FILE_ENV.to_owned(),
            crate::text::path_text(
                &session_root.join("state").join("active-shell.txt"),
                "active shell state path",
            )
            .context("failed to encode active shell state path")?,
        ),
        (
            HELPER_EXECUTABLE_ENV.to_owned(),
            crate::text::path_text(&current_exe, "FuncTerm helper executable")
                .context("failed to encode FuncTerm helper executable")?,
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
                crate::text::path_text(&executable, "shell executable path").with_context(
                    || {
                        format!(
                            "failed to encode {} executable path",
                            shell.canonical_name()
                        )
                    },
                )?,
            ));
        }
    }
    Ok(env)
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
fn prepend_path(shim_dir: &Path) -> Result<String> {
    let mut parts = vec![shim_dir.as_os_str().to_owned()];
    if let Some(path) = std::env::var_os("PATH") {
        parts.extend(std::env::split_paths(&path).map(std::path::PathBuf::into_os_string));
    }
    let joined = std::env::join_paths(parts).context("failed to join PATH entries")?;
    crate::text::os_text(joined, "PATH")
}
#[cfg(test)]
mod tests {
    use super::{ensure_directory, environment};
    use crate::runtime::config::Settings;
    use crate::shell::ShellChoice;
    fn test_settings() -> Settings {
        Settings {
            daemon_service_name: "functerm/test".to_owned(),
            terminal_rows: 30,
            terminal_cols: 120,
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
