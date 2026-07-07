use super::{ShellChoice, shims};
use anyhow::{Context as _, Result, bail};
use std::path::{Path, PathBuf};
pub(super) fn select_available_executable(
    choice: ShellChoice,
    candidates: &[String],
) -> Result<PathBuf> {
    let mut errors = Vec::new();
    for candidate in candidates {
        match resolve_executable(choice, candidate) {
            Ok(path) => return Ok(path),
            Err(error) => errors.push(format!("{candidate}: {error:#}")),
        }
    }
    bail!(
        "none of the configured shell executables are available: {}; {}",
        candidates.join(", "),
        errors.join("; ")
    )
}
fn resolve_executable(choice: ShellChoice, candidate: &str) -> Result<PathBuf> {
    let paths = which::which_all(candidate)?;
    for path in paths {
        if !is_rejected_executable(choice, &path)? {
            return Ok(path);
        }
    }
    bail!("all executable candidates for `{candidate}` are unsuitable")
}
fn is_rejected_executable(choice: ShellChoice, path: &Path) -> Result<bool> {
    Ok(is_inherited_shim(path)? || is_windows_subsystem_bash(choice, path)?)
}
fn is_inherited_shim(path: &Path) -> Result<bool> {
    if same_file(
        path,
        &std::env::current_exe().context("failed to resolve current executable")?,
    ) {
        return Ok(true);
    }
    let Some(shim_dir) = std::env::var_os(shims::SHIM_DIR_ENV) else {
        return Ok(false);
    };
    let Some(parent) = path.parent() else {
        return Ok(false);
    };
    Ok(same_file(parent, &PathBuf::from(shim_dir)))
}
#[cfg(windows)]
fn is_windows_subsystem_bash(choice: ShellChoice, path: &Path) -> Result<bool> {
    if choice != ShellChoice::Bash {
        return Ok(false);
    }
    let Some(system_root) = std::env::var_os("SystemRoot") else {
        bail!("SystemRoot is required to identify WSL Bash");
    };
    Ok(is_windows_subsystem_bash_path(
        path,
        &PathBuf::from(system_root),
    ))
}
#[cfg(not(windows))]
fn is_windows_subsystem_bash(_choice: ShellChoice, _path: &Path) -> Result<bool> {
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
        let system_root = crate::test_fs::temp_case("wsl-bash").join("Windows");
        let system32 = system_root.join("System32");
        std::fs::create_dir_all(&system32).unwrap();
        let bash = system32.join("bash.exe");
        std::fs::write(&bash, b"").unwrap();
        assert!(is_windows_subsystem_bash_path(&bash, &system_root));
    }
    #[cfg(windows)]
    #[test]
    fn non_system32_bash_is_available() {
        let root = crate::test_fs::temp_case("non-wsl-bash");
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
