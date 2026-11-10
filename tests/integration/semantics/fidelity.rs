use crate::support::{
    complete_command, create_tab, locked_with_env, parse_command_result, send_command_with_output,
    tagged_content, temp_dir,
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
#[test]
fn separated_output_replaces_malformed_and_incomplete_utf8_without_losing_the_shell() {
    let _guard = locked_with_env(&[]);
    for (shell, source, next) in [
        (
            "bash",
            "printf 'before\\377after\\342'; printf 'error\\377tail\\342' >&2",
            ":",
        ),
        (
            "python",
            "import sys\n_ = sys.stdout.buffer.write(b'before\\xffafter\\xe2')\n_ = sys.stderr.buffer.write(b'error\\xfftail\\xe2')",
            "pass",
        ),
        (
            "bun",
            "process.stdout.write(Buffer.from('before\\xffafter\\xe2', 'latin1')); process.stderr.write(Buffer.from('error\\xfftail\\xe2', 'latin1')); void 0",
            "void 0",
        ),
    ] {
        let tab = create_tab(&temp_dir("binary-output"), shell).tab_id;
        let result = complete_command(&tab, source);
        assert_eq!(result.exit_code, Some(0_i32), "{shell}: {result:?}");
        assert!(
            result.stdout.contains("before\u{fffd}after\u{fffd}"),
            "{shell}: {result:?}"
        );
        assert!(
            result.stderr.contains("error\u{fffd}tail\u{fffd}"),
            "{shell}: {result:?}"
        );
        let resumed = complete_command(&tab, next);
        assert_eq!(resumed.exit_code, Some(0_i32), "{shell}: {resumed:?}");
    }
}
