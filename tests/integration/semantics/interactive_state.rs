use crate::support::{
    complete_command, create_tab, locked_with_env, parse_command_id, parse_command_result,
    parse_tab_view, run_cli, tagged_content, temp_dir, wait_for_completion,
};
#[test]
fn bash_declarations_arguments_dispatch_and_status_survive_submissions() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("bash-state"), "bash").tab_id;
    complete_command(
        &tab,
        "declare -i n=42; declare -A items=([key]=VALUE); set -- FIRST SECOND; f() { echo USER_FUNCTION; }; false",
    );
    let result = complete_command(
        &tab,
        "printf 'STATUS=%s\\n' \"$?\"; printf '%s\\n' \"$n\" \"${items[key]}\" \"$1\" \"$2\"; f",
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    for expected in [
        "STATUS=1",
        "42",
        "VALUE",
        "FIRST",
        "SECOND",
        "USER_FUNCTION",
    ] {
        assert!(result.stdout.contains(expected), "{result:?}");
    }
}
#[test]
fn python_exception_cache_and_future_parser_match_interactive_execution() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("python-state"), "python").tab_id;
    complete_command(&tab, "raise ValueError('FIRST_EXCEPTION')");
    let first = complete_command(
        &tab,
        "import sys\nprint(type(sys.last_value).__name__, sys.last_value, sys.last_traceback is not None)",
    );
    assert!(
        first.stdout.contains("ValueError FIRST_EXCEPTION True"),
        "{first:?}"
    );
    complete_command(&tab, "raise LookupError('SECOND_EXCEPTION')");
    let next = complete_command(&tab, "print(type(sys.last_value).__name__, sys.last_value)");
    assert!(
        next.stdout.contains("LookupError SECOND_EXCEPTION"),
        "{next:?}"
    );
    complete_command(&tab, "from __future__ import barry_as_FLUFL");
    let accepted = complete_command(&tab, "1 <> 2");
    assert_eq!(accepted.exit_code, Some(0_i32), "{accepted:?}");
    assert!(accepted.stdout.contains("True"), "{accepted:?}");
    let rejected = complete_command(&tab, "1 != 2");
    assert_eq!(rejected.exit_code, Some(1_i32), "{rejected:?}");
}
#[test]
fn abrupt_process_exit_preserves_codes_and_consistent_idle_state() {
    let _guard = locked_with_env(&[]);
    for shell in [
        "python",
        #[cfg(windows)]
        "powershell",
    ] {
        for code in [0_i32, 28_i32] {
            let tab = create_tab(&temp_dir("abrupt-exit"), shell).tab_id;
            let source = if shell == "python" {
                format!("import os; os._exit({code})")
            } else {
                format!("[Environment]::Exit({code})")
            };
            let accepted = crate::support::send_command(&tab, &source, 0.0);
            let id = parse_command_id(&accepted);
            let finished = wait_for_completion(&accepted);
            assert_eq!(finished.exit_code, Some(code), "{finished:?}");
            assert_eq!(
                parse_command_result(&run_cli(&["view", &id])).exit_code,
                Some(code)
            );
            for target in [&id, &tab] {
                let output = run_cli(&["view", target]);
                let document = String::from_utf8(output.stdout).unwrap();
                assert_eq!(tagged_content(&document, "ALIVE").unwrap().trim(), "false");
                assert_eq!(tagged_content(&document, "IDLE").unwrap().trim(), "true");
            }
        }
    }
}
#[cfg(windows)]
#[test]
fn cmd_codepage_changes_do_not_fill_terminal_records_with_blanks() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("codepage-redraw"), "cmd").tab_id;
    complete_command(&tab, "exit /b 28");
    let previous = complete_command(&tab, "echo PREVIOUS=%ERRORLEVEL%");
    assert!(previous.stdout.contains("PREVIOUS=28"), "{previous:?}");
    for codepage in [936_i32, 65_001_i32] {
        complete_command(&tab, &format!("chcp {codepage}"));
        let result = parse_command_result(&run_cli(&[
            "send-command",
            &tab,
            "--command",
            "echo ONE_LINE",
            "--separate-output",
            "false",
            "--wait-timeout",
            "5",
        ]));
        assert_eq!(result.stdout.trim(), "ONE_LINE", "{result:?}");
        assert!(result.stdout.len() < 30, "{result:?}");
    }
    assert!(parse_tab_view(&run_cli(&["view", &tab])).alive);
}
#[cfg(windows)]
#[test]
fn powershell_records_source_locations_native_status_prompts_and_releases_cwd() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("released-directory");
    let tab = create_tab(&cwd, "powershell").tab_id;
    for separate in ["false", "true"] {
        let failed = wait_for_completion(&run_cli(&[
            "send-command",
            &tab,
            "--command",
            "$ErrorView = 'NormalView'\nthrow 'SOURCE_ERROR'",
            "--separate-output",
            separate,
            "--wait-timeout",
            "0",
        ]));
        let diagnostic = format!("{}{}", failed.stdout, failed.stderr);
        assert!(
            diagnostic.contains("SOURCE_ERROR") && diagnostic.contains("CategoryInfo"),
            "{diagnostic}"
        );
        assert!(
            !diagnostic.contains(&cwd.to_string_lossy().into_owned()),
            "{diagnostic}"
        );
        let location = complete_command(
            &tab,
            "$Error[0].InvocationInfo.ScriptLineNumber; $Error[0].InvocationInfo.OffsetInLine",
        );
        assert_eq!(
            location.stdout.trim().lines().collect::<Vec<_>>(),
            ["2", "1"]
        );
    }
    complete_command(&tab, "cmd /c exit 28");
    let previous = complete_command(&tab, "\"PREVIOUS=$LASTEXITCODE\"");
    assert!(previous.stdout.contains("PREVIOUS=28"), "{previous:?}");
    let waiting = crate::support::send_command(&tab, "Read-Host 'VISIBLE_PROMPT'", 0.0);
    let id = parse_command_id(&waiting);
    let deadline = std::time::Instant::now() + core::time::Duration::from_secs(15);
    loop {
        let prompt = parse_command_result(&run_cli(&["view", &id, "--wait-timeout", "1"]));
        if prompt.stdout.contains("VISIBLE_PROMPT") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{prompt:?}");
    }
    crate::support::manual_write(&tab, b"REPLY\r", 0.0);
    assert!(wait_for_completion(&waiting).stdout.contains("REPLY"));
    complete_command(
        &tab,
        "Set-Location -LiteralPath C:/; [Environment]::CurrentDirectory = 'C:/'",
    );
    fs_err::remove_dir(cwd).unwrap();
}
