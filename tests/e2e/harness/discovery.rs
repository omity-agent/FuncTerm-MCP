use std::path::{Path, PathBuf};
pub(crate) fn required(names: &[&str]) -> PathBuf {
    names
        .iter()
        .find_map(|name| real(name))
        .unwrap_or_else(|| panic!("CI must install required executable: {}", names.join(", ")))
}
pub(crate) fn real(name: &str) -> Option<PathBuf> {
    which::which_all(name)
        .ok()?
        .find(|path| !is_functerm_runtime_shim(path) && !is_windows_subsystem_bash(path))
}
pub(crate) fn is_functerm_runtime_shim(path: &Path) -> bool {
    path.parent().is_some_and(is_functerm_shim_directory)
}
pub(crate) fn is_functerm_shim_directory(directory: &Path) -> bool {
    if !directory
        .file_name()
        .is_some_and(|name| names_equal(name, "normal") || names_equal(name, "elevated"))
    {
        return false;
    }
    let Some(shims) = directory.parent() else {
        return false;
    };
    let Some(generation) = named_parent(shims, "shims") else {
        return false;
    };
    let Some(generations) = generation.parent() else {
        return false;
    };
    let Some(service) = named_parent(generations, "generations") else {
        return false;
    };
    let Some(services) = service.parent() else {
        return false;
    };
    let Some(runtime) = named_parent(services, "services") else {
        return false;
    };
    is_runtime_root(runtime)
}
#[cfg(windows)]
fn is_runtime_root(runtime: &Path) -> bool {
    runtime
        .file_name()
        .is_some_and(|name| names_equal(name, "functerm"))
}
#[cfg(unix)]
fn is_runtime_root(runtime: &Path) -> bool {
    let is_user_directory = runtime
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .and_then(|name| name.strip_prefix("functerm-"))
        .is_some_and(|uid| uid.parse::<u32>().is_ok());
    is_user_directory
        && runtime
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| names_equal(name, "functerm"))
}
fn named_parent<'path>(path: &'path Path, name: &str) -> Option<&'path Path> {
    path.file_name()
        .is_some_and(|actual| names_equal(actual, name))
        .then(|| path.parent())
        .flatten()
}
#[cfg(windows)]
fn is_windows_subsystem_bash(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(std::ffi::OsStr::to_str) else {
        return false;
    };
    if !name.eq_ignore_ascii_case("bash.exe") {
        return false;
    }
    let Some(system_root) = std::env::var_os("SystemRoot") else {
        return false;
    };
    path.parent().is_some_and(|parent| {
        parent
            .canonicalize()
            .ok()
            .zip(
                PathBuf::from(system_root)
                    .join("System32")
                    .canonicalize()
                    .ok(),
            )
            .is_some_and(|(actual, system)| actual == system)
    })
}
#[cfg(not(windows))]
const fn is_windows_subsystem_bash(_path: &Path) -> bool {
    false
}
#[cfg(windows)]
fn names_equal(actual: &std::ffi::OsStr, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}
#[cfg(not(windows))]
fn names_equal(actual: &std::ffi::OsStr, expected: &str) -> bool {
    actual == expected
}
