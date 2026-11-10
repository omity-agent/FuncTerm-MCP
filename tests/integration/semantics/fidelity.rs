use crate::support::{
    complete_command, create_tab, locked_with_env, parse_command_result, send_command_with_output,
    tagged_content, temp_dir,
};
#[cfg(windows)]
#[test]
fn powershell_submissions_preserve_interactive_engine_semantics() {
    let _guard = locked_with_env(&[]);
    for separated in [false, true] {
        let tab = create_tab(&temp_dir("interactive-engine"), "powershell").tab_id;
        let submit = |source: &str| {
            crate::support::wait_for_completion(&send_command_with_output(
                &tab, source, separated, 0.0,
            ))
        };
        assert_eq!(submit("$Error.Count").stdout.trim(), "0");
        for source in [
            "[datetime]::Parse('invalid-date')",
            "Invoke-MissingSubmissionCommand",
            "Get-Date -MissingParameter",
            "7 / 0",
        ] {
            let result = submit(&format!("{source}; 'CONTINUED'"));
            assert_eq!(result.exit_code, Some(0_i32), "{source}: {result:?}");
            assert!(result.stdout.contains("CONTINUED"), "{source}: {result:?}");
            let failed = submit(source);
            assert_eq!(failed.exit_code, Some(1_i32), "{source}: {failed:?}");
            assert_eq!(submit("$?").stdout.trim(), "False", "{source}");
        }
        submit("$Error.Clear()");
        let thrown = submit("throw 'SINGLE_RECORD'");
        assert_eq!(thrown.exit_code, Some(1_i32), "{thrown:?}");
        let tokens = submit("\"$^|$$|$($Error.Count)\"");
        assert_eq!(tokens.stdout.trim(), "throw|SINGLE_RECORD|1", "{tokens:?}");
        let history = submit("(Get-History | Select-Object -Last 1).CommandLine");
        assert_eq!(
            history.stdout.trim(),
            "\"$^|$$|$($Error.Count)\"",
            "{history:?}"
        );
        let stack = submit("Get-PSCallStack | ConvertTo-Json -Compress");
        assert!(
            !stack.stdout.contains("FuncTerm") && !stack.stdout.contains(".ps1"),
            "{stack:?}"
        );
        let output = submit("Write-Host 'LEFT' -NoNewline; Write-Host 'RIGHT'");
        assert_eq!(output.stdout.trim(), "LEFTRIGHT", "{output:?}");
        submit("Set-Location Env:");
        let location = submit("(Get-Location).Provider.Name");
        assert_eq!(location.stdout.trim(), "Environment", "{location:?}");
        submit("Set-Location Function:");
        assert_eq!(
            submit("(Get-Location).Provider.Name").stdout.trim(),
            "Function"
        );
        assert_eq!(submit("").exit_code, Some(0_i32));
    }
}
#[cfg(windows)]
#[test]
fn separated_powershell_preserves_diagnostic_streams() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("diagnostic-streams"), "powershell").tab_id;
    let result = complete_command(
        &tab,
        "Write-Output OUTPUT_ONLY; Write-Warning WARNING_MARKER; Write-Verbose VERBOSE_MARKER -Verbose; Write-Debug DEBUG_MARKER -Debug; Write-Information INFORMATION_MARKER -InformationAction Continue; Write-Warning REDIRECTED_WARNING 3>&1; Write-Error REDIRECTED_ERROR 2>&1; Write-Debug HIDDEN_DEBUG -Debug 5>$null",
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    for marker in [
        "OUTPUT_ONLY",
        "INFORMATION_MARKER",
        "REDIRECTED_WARNING",
        "REDIRECTED_ERROR",
    ] {
        assert!(result.stdout.contains(marker), "{result:?}");
        assert!(!result.stderr.contains(marker), "{result:?}");
    }
    assert!(
        !result.stdout.contains("HIDDEN_DEBUG") && !result.stderr.contains("HIDDEN_DEBUG"),
        "{result:?}"
    );
    for marker in ["WARNING_MARKER", "VERBOSE_MARKER", "DEBUG_MARKER"] {
        assert!(result.stderr.contains(marker), "{result:?}");
        assert!(!result.stdout.contains(marker), "{result:?}");
    }
}
#[test]
fn terminal_capture_preserves_spacing_and_wide_character_wraps() {
    let _guard = locked_with_env(&[]);
    let expected = [
        format!("{} B", "A".repeat(119)),
        format!("{} B", "A".repeat(239)),
        format!("{} B", "A".repeat(359)),
        format!("{} B", "A".repeat(4999)),
        format!("{}界B", "A".repeat(119)),
        format!("{}界B", "A".repeat(239)),
        format!("{}A界B", "界".repeat(59)),
        "e\u{301} 中文 🧠🚀".to_owned(),
    ];
    let text = format!(
        "\x1b[2J\x1b[H\x1b]2;终端标题\x1b\\\x1b[31m{}\nOVERWRITTEN\r\x1b[2KFINAL_FRAME\x1b[0m",
        expected.join("\n")
    );
    for shell in [
        "bun",
        #[cfg(windows)]
        "powershell",
    ] {
        let tab = create_tab(&temp_dir("wide-wrap"), shell).tab_id;
        let source = if shell == "powershell" {
            format!(
                "Write-Output {}",
                functerm::shell::quote::powershell_string(&text)
            )
        } else {
            format!("console.log({})", sonic_rs::to_string(&text).unwrap())
        };
        let response = send_command_with_output(&tab, &source, false, 5.0);
        let result = parse_command_result(&response);
        assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
        let document = String::from_utf8(response.stdout).unwrap();
        assert_eq!(tagged_content(&document, "TITLE"), Some("\n终端标题\n"));
        assert!(!result.stdout.contains("OVERWRITTEN"), "{result:?}");
        assert!(result.stdout.contains("FINAL_FRAME"), "{result:?}");
        for line in &expected {
            assert!(result.stdout.contains(line), "{shell}: {result:?}");
        }
    }
}
#[test]
fn separated_output_replaces_malformed_and_incomplete_utf8_without_losing_the_shell() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("binary-output"), "bash").tab_id;
    let result = complete_command(
        &tab,
        "printf 'before\\377after\\342'; printf 'error\\377tail\\342' >&2",
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert_eq!(result.stdout.trim(), "before\u{fffd}after\u{fffd}");
    assert_eq!(result.stderr.trim(), "error\u{fffd}tail\u{fffd}");
    let resumed = complete_command(&tab, ":");
    assert_eq!(resumed.exit_code, Some(0_i32), "{resumed:?}");
}
