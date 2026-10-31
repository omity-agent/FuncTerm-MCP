use crate::support::{
    complete_command, create_tab, locked_with_env, parse_command_result, run_cli, temp_dir,
};
#[cfg(windows)]
#[test]
fn separated_powershell_preserves_diagnostic_streams() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("diagnostic-streams"), "powershell").tab_id;
    let result = complete_command(
        &tab,
        "Write-Output OUTPUT_ONLY; Write-Warning WARNING_MARKER; Write-Verbose VERBOSE_MARKER -Verbose; Write-Debug DEBUG_MARKER -Debug; Write-Information INFORMATION_MARKER -InformationAction Continue; Write-Warning REDIRECTED_WARNING 3>&1; Write-Debug HIDDEN_DEBUG -Debug 5>$null",
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    for marker in ["OUTPUT_ONLY", "INFORMATION_MARKER", "REDIRECTED_WARNING"] {
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
        format!("{}界B", "A".repeat(119)),
        format!("{}界B", "A".repeat(239)),
        format!("{}A界B", "界".repeat(59)),
    ];
    let text = expected.join("\n");
    for shell in ["bun", if cfg!(windows) { "powershell" } else { "bun" }] {
        let tab = create_tab(&temp_dir("wide-wrap"), shell).tab_id;
        let source = if shell == "powershell" {
            format!(
                "Write-Output {}",
                functerm::shell::quote::powershell_string(&text)
            )
        } else {
            format!("console.log({})", sonic_rs::to_string(&text).unwrap())
        };
        let result = parse_command_result(&run_cli(&[
            "send-command",
            &tab,
            "--command",
            &source,
            "--separate-output",
            "false",
            "--wait-timeout",
            "5",
        ]));
        assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
        for line in &expected {
            assert!(result.stdout.contains(line), "{shell}: {result:?}");
        }
    }
}
#[cfg(windows)]
#[test]
fn bash_reports_native_cwd_for_apostrophes() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("cwd-format").join("apostrophe'case");
    fs_err::create_dir_all(&cwd).unwrap();
    let tab = create_tab(cwd.parent().unwrap(), "bash").tab_id;
    let source = format!(
        "cd {}",
        functerm::shell::quote::posix_string(&cwd.to_str().unwrap().replace('\\', "/"))
    );
    let result = complete_command(&tab, &source);
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert_eq!(
        result.cwd.replace('\\', "/"),
        cwd.to_str().unwrap().replace('\\', "/")
    );
    let next = complete_command(&tab, "pwd");
    assert_eq!(next.exit_code, Some(0_i32), "{next:?}");
}
