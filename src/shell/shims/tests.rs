use super::{SHIM_DIR_ENV, environment};
use crate::runtime::protocol::EnvironmentSnapshot;
use crate::shell::ShellChoice;
use std::ffi::OsString;
#[test]
fn environment_replaces_inherited_shim_path_and_internal_state() {
    let root = crate::test_fs::temp_dir("shim-environment-replacement");
    let session_root = root.join("session");
    let old_shim = root.join("old-shims");
    let new_shim = root.join("new-shims");
    let tool_dir = root.join("tools");
    let inherited_path = std::env::join_paths([&old_shim, &tool_dir]).unwrap();
    let inherited = EnvironmentSnapshot::from_variables([
        (OsString::from("PATH"), inherited_path),
        (OsString::from(SHIM_DIR_ENV), old_shim.into_os_string()),
        (
            OsString::from("FUNCTERM_SESSION_ROOT"),
            OsString::from("stale-session"),
        ),
        (OsString::from("UNMANAGED_MARKER"), OsString::from("kept")),
    ]);
    let env = environment(
        &super::tests::test_settings(),
        &session_root,
        &new_shim,
        ShellChoice::PowerShell,
        &inherited,
        &root,
    )
    .unwrap();
    let path = env
        .iter()
        .find(|pair| pair.0 == "PATH")
        .map(|pair| pair.1.clone())
        .unwrap();
    let entries = std::env::split_paths(&path).collect::<Vec<_>>();
    assert_eq!(entries, vec![new_shim, tool_dir]);
    assert!(
        env.iter()
            .any(|pair| pair.0 == "UNMANAGED_MARKER" && pair.1 == "kept")
    );
    assert!(
        env.iter().any(|pair| {
            pair.0 == "FUNCTERM_SESSION_ROOT" && pair.1 == session_root.as_os_str()
        })
    );
}
