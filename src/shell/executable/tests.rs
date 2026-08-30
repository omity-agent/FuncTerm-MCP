#[cfg(windows)]
use super::{is_windows_subsystem_bash_path, select_available_executable};
#[cfg(windows)]
use crate::runtime::protocol::EnvironmentSnapshot;
#[cfg(windows)]
use crate::shell::{ShellChoice, shims};
#[cfg(windows)]
use std::ffi::OsString;
#[cfg(windows)]
use std::path::Path;
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
        Path::new("C:\\Windows")
    ));
}
#[cfg(windows)]
#[test]
fn resolution_uses_snapshot_pathext_after_rejected_shim() {
    let root = crate::test_fs::temp_dir("snapshot-pathext");
    let shim_directory = root.join("shims");
    let real_directory = root.join("real");
    std::fs::create_dir_all(&shim_directory).unwrap();
    std::fs::create_dir_all(&real_directory).unwrap();
    let current_executable = std::env::current_exe().unwrap();
    std::fs::copy(&current_executable, shim_directory.join("bun")).unwrap();
    let extension = ".FUNCTERM_SNAPSHOT_TEST";
    let real_executable = real_directory.join(format!("bun{extension}"));
    std::fs::copy(current_executable, &real_executable).unwrap();
    assert!(
        !std::env::var("PATHEXT")
            .unwrap_or_default()
            .split(';')
            .any(|entry| entry.eq_ignore_ascii_case(extension))
    );
    let path = std::env::join_paths([&shim_directory, &real_directory]).unwrap();
    let environment = EnvironmentSnapshot::from_variables([
        (OsString::from("PATH"), path),
        (OsString::from("PATHEXT"), OsString::from(extension)),
        (
            OsString::from(shims::SHIM_DIR_ENV),
            shim_directory.into_os_string(),
        ),
    ]);
    let resolved =
        select_available_executable(ShellChoice::Bun, &["bun".to_owned()], &environment, &root)
            .unwrap();
    assert_eq!(
        resolved.canonicalize().unwrap(),
        real_executable.canonicalize().unwrap()
    );
}
