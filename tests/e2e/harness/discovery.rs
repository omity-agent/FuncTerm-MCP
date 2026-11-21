use std::path::{Path, PathBuf};
pub(crate) fn required(names: &[&str]) -> PathBuf {
    names
        .iter()
        .find_map(|name| {
            which::which_all(name).ok()?.find(|path| {
                !path.parent().is_some_and(is_functerm_shim_directory)
                    && !is_windows_subsystem_bash(path)
            })
        })
        .unwrap_or_else(|| panic!("CI must install required executable: {}", names.join(", ")))
}
pub(crate) fn is_functerm_shim_directory(directory: &Path) -> bool {
    let suffix = std::env::consts::EXE_SUFFIX;
    let entrypoint = directory.join(format!("functerm{suffix}"));
    let bash = directory.join(format!("bash{suffix}"));
    same_file::is_same_file(entrypoint, bash).unwrap_or(false)
}
#[cfg(windows)]
fn is_windows_subsystem_bash(path: &Path) -> bool {
    let Some(root) = std::env::var_os("SystemRoot") else {
        return false;
    };
    same_file::is_same_file(path, PathBuf::from(root).join("System32/bash.exe")).unwrap_or(false)
}
#[cfg(not(windows))]
const fn is_windows_subsystem_bash(_path: &Path) -> bool {
    false
}
