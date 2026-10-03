use crate::support::{
    locked_with_env, parse_command_result, parse_tab_created, run_cli, send_command, temp_dir,
};
#[test]
fn cli_profile_policy_is_per_tab_and_inherited_by_nested_shells() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("profile-policy");
    #[cfg(windows)]
    let (shell, query, nested) = (
        "powershell",
        "Write-Output $env:FUNCTERM_LOAD_PROFILE",
        "pwsh -NoLogo",
    );
    #[cfg(unix)]
    let (shell, query, nested) = (
        "bash",
        "printf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"",
        "bash -i",
    );
    for (no_profile, expected) in [(false, "1"), (true, "0")] {
        let mut arguments = vec![
            "new-tab",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            shell,
        ];
        if no_profile {
            arguments.push("--no-profile");
        }
        let output = run_cli(&arguments);
        assert!(output.status.success(), "{output:?}");
        let tab = parse_tab_created(&output);
        assert_policy(&tab.tab_id, query, expected);
        let switched = parse_command_result(&send_command(&tab.tab_id, nested, 10.0));
        assert!(switched.finished, "{}", switched.stderr);
        assert_policy(&tab.tab_id, query, expected);
    }
}
fn assert_policy(tab_id: &str, query: &str, expected: &str) {
    let result = parse_command_result(&send_command(tab_id, query, 10.0));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32), "{}", result.stderr);
    assert_eq!(result.stdout.trim(), expected);
}
