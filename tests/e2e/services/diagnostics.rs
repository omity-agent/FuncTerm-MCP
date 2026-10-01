use crate::support::{
    create_tab, locked, parse_command_result, parse_tab_view, run_cli, send_command, temp_root,
};
#[test]
fn powershell_user_errors_hide_wrapper_details_and_keep_tab_usable() {
    let _guard = locked();
    let tab = create_tab(&temp_root(), "powershell");
    for (command, diagnostic) in [
        (
            r#"rg --line-number "FileLinkMenuOpen|useSourceHover|HighlightedCode|useSyncExternalStore|addEventListener\(\"(keydown|keyup)" src/app/frontend tests/app/frontend"#,
            "(keydown|keyup)",
        ),
        (
            "Write-Output 'FUNCTERM_SHOULD_NOT_EXECUTE'\nWrite-Output \"unterminated",
            "unterminated",
        ),
        (
            "throw 'FUNCTERM_USER_RUNTIME_ERROR'",
            "FUNCTERM_USER_RUNTIME_ERROR",
        ),
    ] {
        let output = send_command(&tab.tab_id, command, 5.0);
        let result = parse_command_result(&output);
        assert!(result.finished, "command should finish: {}", result.stderr);
        assert_eq!(result.exit_code, Some(1_i32), "{}", result.stderr);
        assert!(result.stdout.trim().is_empty(), "{}", result.stdout);
        assert!(result.stderr.contains(diagnostic), "{}", result.stderr);
        let view = parse_tab_view(&run_cli(&["view", &tab.tab_id]));
        assert!(view.alive, "user errors should not close the tab");
        for text in [&result.stderr, &view.screen] {
            for internal in [
                "powershell_init.ps1",
                "commandScript_",
                "[scriptblock]::Create",
                "MethodInvocationException",
            ] {
                assert!(
                    !text.contains(internal),
                    "user error exposed {internal}:\n{text}"
                );
            }
        }
    }
    let recovered = parse_command_result(&send_command(
        &tab.tab_id,
        "Write-Output 'FUNCTERM_AFTER_USER_ERROR'",
        5.0,
    ));
    assert!(recovered.finished);
    assert_eq!(recovered.exit_code, Some(0_i32), "{}", recovered.stderr);
    assert!(recovered.stdout.contains("FUNCTERM_AFTER_USER_ERROR"));
    assert!(recovered.stderr.is_empty(), "{}", recovered.stderr);
}
#[test]
fn powershell_continues_after_command_not_found_and_preserves_state() {
    let _guard = locked();
    let tab = create_tab(&temp_root(), "powershell");
    let result = parse_command_result(&send_command(
        &tab.tab_id,
        "$ErrorActionPreference = 'Continue'; FuncTermMissingCommand_8d4f; $FuncTermStateAfterError = 'FUNCTERM_STATE_AFTER_ERROR'; Write-Output 'FUNCTERM_AFTER_COMMAND_NOT_FOUND'; Start-Sleep -Milliseconds 25",
        5.0,
    ));
    assert!(result.finished, "command should finish: {}", result.stderr);
    assert_eq!(result.exit_code, Some(0_i32), "{}", result.stderr);
    assert!(
        result.stdout.contains("FUNCTERM_AFTER_COMMAND_NOT_FOUND"),
        "{}",
        result.stdout
    );
    assert!(result.stderr.contains("FuncTermMissingCommand_8d4f"));
    assert_ne!(result.time_consumption, "0.00ms");
    let state = parse_command_result(&send_command(
        &tab.tab_id,
        "Write-Output $FuncTermStateAfterError",
        5.0,
    ));
    assert_eq!(state.exit_code, Some(0_i32), "{}", state.stderr);
    assert!(state.stdout.contains("FUNCTERM_STATE_AFTER_ERROR"));
}
