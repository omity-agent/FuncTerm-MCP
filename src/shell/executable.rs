use super::{ShellChoice, shims};
use crate::runtime::protocol::EnvironmentSnapshot;
use anyhow::{Context as _, Result, bail};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
pub(super) fn select_available_executable(
    choice: ShellChoice,
    candidates: &[String],
    environment: &EnvironmentSnapshot,
    cwd: &Path,
) -> Result<PathBuf> {
    let search_path = environment.value("PATH");
    let inherited_shim = environment.value(shims::SHIM_DIR_ENV);
    let mut errors = Vec::new();
    for candidate in candidates {
        match resolve_executable(
            choice,
            candidate,
            search_path.as_deref(),
            cwd,
            inherited_shim.as_deref(),
            environment,
        ) {
            Ok(executable) => return Ok(executable),
            Err(error) => errors.push(format!("{candidate}: {error:#}")),
        }
    }
    bail!(
        "none of the configured shell executables are available: {}; {}",
        candidates.join(", "),
        errors.join("; ")
    )
}
fn resolve_executable(
    choice: ShellChoice,
    candidate: &str,
    path: Option<&OsStr>,
    cwd: &Path,
    inherited_shim: Option<&OsStr>,
    environment: &EnvironmentSnapshot,
) -> Result<PathBuf> {
    let executables = which::which_in_all(candidate, path, cwd)?;
    for executable in executables {
        if !is_rejected_executable(choice, &executable, inherited_shim, environment)? {
            return Ok(executable);
        }
    }
    bail!("all executable candidates for `{candidate}` are unsuitable")
}
fn is_rejected_executable(
    choice: ShellChoice,
    path: &Path,
    inherited_shim: Option<&OsStr>,
    environment: &EnvironmentSnapshot,
) -> Result<bool> {
    Ok(is_inherited_shim(path, inherited_shim)?
        || is_windows_subsystem_bash(choice, path, environment)?)
}
fn is_inherited_shim(path: &Path, inherited_shim: Option<&OsStr>) -> Result<bool> {
    if same_file(
        path,
        &std::env::current_exe().context("failed to resolve current executable")?,
    ) {
        return Ok(true);
    }
    let Some(shim_dir) = inherited_shim else {
        return Ok(false);
    };
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    let configured_shim = PathBuf::from(shim_dir);
    Ok(path_equals(parent, &configured_shim) || same_file(parent, &configured_shim))
}
#[cfg(windows)]
fn path_equals(left: &Path, right: &Path) -> bool {
    left.as_os_str().eq_ignore_ascii_case(right.as_os_str())
}
#[cfg(not(windows))]
fn path_equals(left: &Path, right: &Path) -> bool {
    left == right
}
#[cfg(windows)]
fn is_windows_subsystem_bash(
    choice: ShellChoice,
    path: &Path,
    environment: &EnvironmentSnapshot,
) -> Result<bool> {
    if choice != ShellChoice::Bash {
        return Ok(false);
    }
    let Some(system_root) = environment.value("SystemRoot") else {
        bail!("SystemRoot is required to identify WSL Bash");
    };
    Ok(is_windows_subsystem_bash_path(
        path,
        &PathBuf::from(system_root),
    ))
}
#[cfg(not(windows))]
fn is_windows_subsystem_bash(
    _choice: ShellChoice,
    _path: &Path,
    _environment: &EnvironmentSnapshot,
) -> Result<bool> {
    Ok(false)
}
#[cfg(windows)]
fn is_windows_subsystem_bash_path(path: &Path, system_root: &Path) -> bool {
    executable_name_equals(path, "bash.exe")
        && path
            .parent()
            .is_some_and(|parent| same_file(parent, &system_root.join("System32")))
}
#[cfg(windows)]
fn executable_name_equals(path: &Path, expected: &str) -> bool {
    path.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|name| name.eq_ignore_ascii_case(expected))
}
fn same_file(left: &Path, right: &Path) -> bool {
    let Ok(left_path) = left.canonicalize() else {
        return false;
    };
    let Ok(right_path) = right.canonicalize() else {
        return false;
    };
    left_path == right_path
}
#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::is_windows_subsystem_bash_path;
    #[cfg(windows)]
    #[test]
    fn windows_system32_bash_is_rejected_as_wsl() {
        let system_root = crate::test_fs::temp_dir("wsl-bash").join("Windows");
        let system32 = system_root.join("System32");
        std::fs::create_dir_all(&system32).unwrap();
        let bash = system32.join("bash.exe");
        std::fs::write(&bash, b"").unwrap();
        assert!(is_windows_subsystem_bash_path(&bash, &system_root));
    }
    #[cfg(windows)]
    #[test]
    fn non_system32_bash_is_available() {
        let root = crate::test_fs::temp_dir("non-wsl-bash");
        let directory = root.join("Git").join("bin");
        std::fs::create_dir_all(&directory).unwrap();
        let bash = directory.join("bash.exe");
        std::fs::write(&bash, b"").unwrap();
        assert!(!is_windows_subsystem_bash_path(
            &bash,
            std::path::Path::new("C:\\Windows")
        ));
    }
}
