use crate::support::{
    complete_command, create_tab, locked_with_env, parse_command_result, run_cli, temp_dir,
};
#[test]
fn bun_await_preserves_multiline_execution_and_persistent_bindings() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("await-order"), "bun").tab_id;
    let result = complete_command(
        &tab,
        "var started = performance.now();\nvar resolved;\nawait new Promise(resolve => setTimeout(() => { resolved = 42; resolve(); }, 1200));\nconsole.log('ORDER', resolved, performance.now() - started >= 1100);",
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert!(result.stdout.contains("ORDER 42 true"), "{result:?}");
    let next = complete_command(&tab, "console.log('RETAINED', resolved)");
    assert!(next.stdout.contains("RETAINED 42"), "{next:?}");
}
#[cfg(windows)]
#[test]
fn shim_preserves_native_quoting_and_full_exit_status() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("shim-transparency"), "powershell").tab_id;
    let quoted = complete_command(
        &tab,
        "cmd.exe /d /c 'echo \"A & echo UNEXPECTED_SECOND_COMMAND\"'",
    );
    assert_eq!(
        quoted.stdout.trim(),
        "\"A & echo UNEXPECTED_SECOND_COMMAND\"",
        "{quoted:?}"
    );
    for executable in ["cmd.exe /d /c", "pwsh -NoProfile -Command"] {
        for code in [3_010_i32, -1_i32] {
            let result = complete_command(&tab, &format!("{executable} 'exit {code}'"));
            assert_eq!(result.exit_code, Some(code), "{result:?}");
            assert!(result.stderr.is_empty(), "{result:?}");
        }
    }
}
#[cfg(windows)]
#[test]
fn cmd_submission_uses_interactive_percent_and_batch_return_semantics() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("interactive-cmd");
    fs_err::write(
        cwd.join("return-code.cmd"),
        "@echo BATCH_CALLED\r\n@exit /b 23\r\n",
    )
    .unwrap();
    let tab = create_tab(&cwd, "cmd").tab_id;
    for source in ["cd /d C:\\", "cd /", "echo AFTER_ROOT"] {
        let result = parse_command_result(&run_cli(&[
            "send-command",
            &tab,
            "--command",
            source,
            "--wait-timeout",
            "5",
        ]));
        assert!(result.finished, "{result:?}");
        assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    }
    complete_command(&tab, &format!("cd /d \"{}\"", cwd.display()));
    for source in [
        "echo BEFORE_CALL\ncall return-code.cmd\necho AFTER_CALL",
        "echo BEFORE_SECOND\ncall return-code.cmd\necho AFTER_SECOND",
        "echo PERCENT=100%%\necho ARGUMENTS=%0_%1",
    ] {
        let result = complete_command(&tab, source);
        assert_eq!(result.exit_code, Some(23_i32), "{result:?}");
        if source.starts_with("echo PERCENT") {
            assert!(result.stdout.contains("PERCENT=100%%"), "{result:?}");
            assert!(result.stdout.contains("ARGUMENTS=%0_%1"), "{result:?}");
        } else {
            assert!(result.stdout.contains("BATCH_CALLED"), "{result:?}");
            assert!(result.stdout.contains("AFTER_"), "{result:?}");
        }
    }
    let returned = complete_command(&tab, "call return-code.cmd");
    assert_eq!(returned.exit_code, Some(23_i32), "{returned:?}");
}
