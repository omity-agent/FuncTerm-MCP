use crate::support::{
    create_tab, locked, parse_command_result, parse_tab_view, run_cli, send_command, temp_root,
};
#[test]
fn powershell_errors_preserve_diagnostics_state_and_a_usable_prompt() {
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
        let result = parse_command_result(&send_command(&tab.tab_id, command, 5.0));
        assert!(result.finished, "{result:?}");
        assert_eq!(result.exit_code, Some(1_i32), "{result:?}");
        assert!(result.stdout.trim().is_empty(), "{result:?}");
        assert!(result.stderr.contains(diagnostic), "{result:?}");
        let view = parse_tab_view(&run_cli(&["view", &tab.tab_id]));
        assert!(view.alive);
        for text in [&result.stderr, &view.screen] {
            for internal in [
                "powershell_init.ps1",
                "commandScript_",
                "[scriptblock]::Create",
                "MethodInvocationException",
            ] {
                assert!(!text.contains(internal), "exposed {internal}: {text}");
            }
        }
    }
    for separate in ["true", "false"] {
        for (statement, diagnostic) in [
            ("FuncTermMissingCommand_8d4f", "FuncTermMissingCommand_8d4f"),
            (
                "'not-base64!' | ForEach-Object { [Convert]::FromBase64String($_) }",
                "Base-64",
            ),
            (
                "[int]::Parse('FUNCTERM_INVALID_INTEGER')",
                "FUNCTERM_INVALID_INTEGER",
            ),
            (
                "Get-Item -FuncTermUnknownParameter",
                "FuncTermUnknownParameter",
            ),
        ] {
            let command = format!(
                "$ErrorActionPreference = 'Continue'\n{statement}\n$FuncTermStateAfterError = 'FUNCTERM_STATE_AFTER_ERROR'\nWrite-Output 'FUNCTERM_AFTER_STATEMENT_ERROR'"
            );
            let response = run_cli(&[
                "send-command",
                &tab.tab_id,
                "--command",
                &command,
                "--separate-output",
                separate,
                "--wait-timeout",
                "5",
            ]);
            assert!(response.status.success(), "{response:?}");
            let text = String::from_utf8(response.stdout.clone()).unwrap();
            for expected in [
                "<FINISHED>\ntrue\n",
                "<EXIT_CODE>\n0\n",
                "FUNCTERM_AFTER_STATEMENT_ERROR",
                diagnostic,
            ] {
                assert!(text.contains(expected), "{statement}: {text}");
            }
            if separate == "true" {
                assert!(
                    parse_command_result(&response).stderr.contains(diagnostic),
                    "{text}"
                );
            }
            assert_recovered(
                &tab.tab_id,
                "Write-Output $FuncTermStateAfterError",
                "FUNCTERM_STATE_AFTER_ERROR",
            );
        }
    }
    for (statement, diagnostic) in [
        ("throw 'FUNCTERM_EXPLICIT_THROW'", "FUNCTERM_EXPLICIT_THROW"),
        (
            "Write-Error 'FUNCTERM_PRIOR_ERROR'\nthrow 'FUNCTERM_FINAL_THROW'",
            "FUNCTERM_FINAL_THROW",
        ),
        (
            "Write-Error 'FUNCTERM_EXPLICIT_STOP' -ErrorAction Stop",
            "FUNCTERM_EXPLICIT_STOP",
        ),
        (
            "$ErrorActionPreference = 'Stop'\n[Convert]::FromBase64String('not-base64!')",
            "Base-64",
        ),
    ] {
        let command = format!(
            "Start-Sleep -Milliseconds 50\n{statement}\nWrite-Output 'FUNCTERM_SHOULD_NOT_EXECUTE'"
        );
        let result = parse_command_result(&send_command(&tab.tab_id, &command, 5.0));
        assert!(result.finished, "{result:?}");
        assert_eq!(result.exit_code, Some(1_i32), "{result:?}");
        assert!(result.stdout.trim().is_empty(), "{result:?}");
        assert!(result.stderr.contains(diagnostic), "{result:?}");
        assert!(
            humantime::parse_duration(&result.time_consumption).unwrap()
                >= core::time::Duration::from_millis(25),
            "{result:?}"
        );
        assert_recovered(
            &tab.tab_id,
            "$ErrorActionPreference = 'Continue'\nWrite-Output RECOVERED_AFTER_TERMINATION",
            "RECOVERED_AFTER_TERMINATION",
        );
    }
}
fn assert_recovered(tab: &str, source: &str, marker: &str) {
    let result = parse_command_result(&send_command(tab, source, 5.0));
    assert!(result.finished, "{result:?}");
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert!(result.stdout.contains(marker), "{result:?}");
    assert!(result.stderr.is_empty(), "{result:?}");
}
