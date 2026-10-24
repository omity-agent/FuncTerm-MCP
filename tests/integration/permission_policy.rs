use crate::support::{
    create_tab, locked_with_env, parse_command_result, parse_tab_created, run_cli, send_command,
    temp_dir,
};
#[cfg(windows)]
pub(crate) const SHELL: &str = "powershell";
#[cfg(unix)]
pub(crate) const SHELL: &str = "bash";
#[cfg(windows)]
pub(crate) const QUERY: &str = "[Security.Principal.WindowsPrincipal]::new([Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)";
#[cfg(unix)]
pub(crate) const QUERY: &str = "printf '%s\\n' \"$EUID\"";
#[test]
fn cli_tabs_default_to_normal_and_require_daemon_permission_for_elevation() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("normal-permissions");
    let tab = create_tab(&directory, SHELL);
    assert_tab_permission(&tab.tab_id, false);
    assert!(
        run_cli(&["close", "--tab-id", &tab.tab_id])
            .status
            .success()
    );
    let output = run_cli(&[
        "new-tab",
        "--no-profile",
        "--starting-shell",
        SHELL,
        "--starting-directory",
        directory.to_str().unwrap(),
        "--elevated",
    ]);
    if daemon_privileged() {
        assert!(output.status.success(), "{output:?}");
        let elevated = parse_tab_created(&output);
        assert_tab_permission(&elevated.tab_id, true);
        assert!(
            run_cli(&["close", "--tab-id", &elevated.tab_id])
                .status
                .success()
        );
    } else {
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("requires the Daemon"),
            "{output:?}"
        );
    }
}
fn assert_tab_permission(tab_id: &str, expected: bool) {
    let result = parse_command_result(&send_command(tab_id, QUERY, 10.0));
    assert_eq!(result.exit_code, Some(0_i32), "{}", result.stderr);
    assert!(result.finished);
    assert_eq!(
        privileged_output(&result.stdout),
        expected,
        "{}",
        result.stdout
    );
}
#[cfg(windows)]
pub(crate) fn privileged_output(output: &str) -> bool {
    match output.trim() {
        "True" => true,
        "False" => false,
        unexpected => panic!("unexpected Windows privilege query: {unexpected}"),
    }
}
#[cfg(unix)]
pub(crate) fn privileged_output(output: &str) -> bool {
    output.trim().parse::<u32>().unwrap() == 0
}
#[cfg(unix)]
pub(crate) fn daemon_privileged() -> bool {
    rustix::process::geteuid().is_root()
}
#[cfg(windows)]
pub(crate) fn daemon_privileged() -> bool {
    use std::os::windows::io::{FromRawHandle as _, OwnedHandle};
    use windows::Win32::{
        GetCurrentProcess, GetTokenInformation, OpenProcessToken, TOKEN_ELEVATION, TOKEN_QUERY,
        TokenElevation,
    };
    let process = unsafe { GetCurrentProcess() };
    let mut handle = core::ptr::null_mut();
    unsafe {
        OpenProcessToken(
            process,
            u32::try_from(TOKEN_QUERY).unwrap(),
            &raw mut handle,
        )
    }
    .ok()
    .unwrap();
    let _token = unsafe { OwnedHandle::from_raw_handle(handle) };
    let mut elevation = TOKEN_ELEVATION::default();
    let mut returned = 0;
    unsafe {
        GetTokenInformation(
            handle,
            TokenElevation,
            Some(core::ptr::from_mut(&mut elevation).cast()),
            u32::try_from(core::mem::size_of_val(&elevation)).unwrap(),
            &raw mut returned,
        )
    }
    .ok()
    .unwrap();
    elevation.TokenIsElevated != 0
}
