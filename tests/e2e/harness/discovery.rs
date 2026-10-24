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
    let names = directory
        .ancestors()
        .take(6)
        .filter_map(Path::file_name)
        .collect::<Vec<_>>();
    let &[privilege, shims, _, generations, _, services] = names.as_slice() else {
        return false;
    };
    let equals = |actual: &std::ffi::OsStr, expected: &str| {
        if cfg!(windows) {
            actual.eq_ignore_ascii_case(expected)
        } else {
            actual == expected
        }
    };
    (equals(privilege, "normal") || equals(privilege, "elevated"))
        && equals(shims, "shims")
        && equals(generations, "generations")
        && equals(services, "services")
        && directory.ancestors().nth(6).is_some_and(is_runtime_root)
}
#[cfg(windows)]
fn is_runtime_root(runtime: &Path) -> bool {
    runtime
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("functerm"))
}
#[cfg(unix)]
fn is_runtime_root(runtime: &Path) -> bool {
    runtime
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|name| name.strip_prefix("functerm-"))
        .is_some_and(|uid| uid.parse::<u32>().is_ok())
        && runtime
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "functerm")
}
#[cfg(windows)]
fn is_windows_subsystem_bash(path: &Path) -> bool {
    let Some(root) = std::env::var_os("SystemRoot") else {
        return false;
    };
    path.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("bash.exe"))
        && path.parent().is_some_and(|parent| {
            parent
                .canonicalize()
                .ok()
                .zip(PathBuf::from(root).join("System32").canonicalize().ok())
                .is_some_and(|(actual, system)| actual == system)
        })
}
#[cfg(not(windows))]
const fn is_windows_subsystem_bash(_path: &Path) -> bool {
    false
}
