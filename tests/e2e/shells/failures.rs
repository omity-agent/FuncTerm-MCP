use crate::support::{
    complete_command, create_tab, parse_command_id, parse_command_result, parse_tab_view, run_cli,
    send_command, temp_dir, wait_for_completion,
};
use core::time::Duration;
#[test]
fn nushell_parse_errors_and_user_bindings_preserve_lexical_state() {
    let _guard = super::matrix::daemon();
    let tab = create_tab(&temp_dir("parse-errors"), "nu").tab_id;
    let defined = complete_command(&tab, "let retained = 41");
    assert_eq!(defined.exit_code, Some(0_i32), "{defined:?}");
    for source in ["let x =", "def broken [] {", "print $unknown_variable"] {
        let failed = complete_command(&tab, source);
        assert!(failed.finished, "{failed:?}");
        assert_ne!(failed.exit_code, Some(0_i32), "{failed:?}");
        let next = complete_command(&tab, "print ($retained + 1)");
        assert_eq!(next.exit_code, Some(0_i32), "{next:?}");
        assert!(next.stdout.contains("42"), "{next:?}");
    }
    let bindings = complete_command(
        &tab,
        "let FUNCTERM_NU_ACTIVE = 'OVERRIDDEN'; let FUNCTERM_HELPER_PATH = 'OVERRIDDEN'; let FUNCTERM_RECOVERY_ENVIRONMENT = 'OVERRIDDEN'",
    );
    assert_eq!(bindings.exit_code, Some(0_i32), "{bindings:?}");
    for source in ["print AFTER_BINDINGS", "hide-env FUNCTERM_SESSION_ROOT"] {
        let result = complete_command(&tab, source);
        assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    }
    let next = complete_command(&tab, "print $FUNCTERM_NU_ACTIVE");
    assert_eq!(next.exit_code, Some(0_i32), "{next:?}");
    assert!(next.stdout.contains("OVERRIDDEN"), "{next:?}");
}
#[test]
fn powershell_top_level_return_keeps_state_and_does_not_exit() {
    let _guard = super::matrix::daemon();
    for separated in ["false", "true"] {
        let tab = create_tab(&temp_dir("early-return"), "powershell").tab_id;
        let output = run_cli(&[
            "send-command",
            &tab,
            "--command",
            "$retained = 42; function f { 'USER_F' }; function Get-Retained { $retained }; return 'RETURN_VALUE'; throw 'UNREACHABLE'",
            "--separate-output",
            separated,
            "--wait-timeout",
            "0",
        ]);
        let returned = wait_for_completion(&output);
        assert_eq!(returned.exit_code, Some(0_i32), "{returned:?}");
        assert!(returned.stdout.contains("RETURN_VALUE"), "{returned:?}");
        let next = complete_command(&tab, "f; Get-Retained");
        assert_eq!(next.exit_code, Some(0_i32), "{next:?}");
        assert!(
            next.stdout.contains("42") && next.stdout.contains("USER_F"),
            "{next:?}"
        );
        #[cfg(windows)]
        assert_eq!(
            complete_command(&tab, "cmd /c exit 7; Write-Output OK").exit_code,
            Some(0_i32)
        );
        let header = complete_command(
            &tab,
            "using namespace System.Text\nparam([int]$number = 42)\nend { return [StringBuilder]::new('HEADER_' + $number).ToString() }",
        );
        assert_eq!(header.exit_code, Some(0_i32), "{header:?}");
        assert!(header.stdout.contains("HEADER_42"), "{header:?}");
    }
}
#[test]
fn powershell_separated_stdout_preserves_interleaved_write_order() {
    let _guard = super::matrix::daemon();
    let tab = create_tab(&temp_dir("ordered-streams"), "powershell").tab_id;
    let result = complete_command(
        &tab,
        r#"[Console]::WriteLine('FIRST'); Write-Output 'SECOND'; [Console]::WriteLine('THIRD'); Write-Output 'FOURTH'; Write-Host 'FIFTH'; $childInfo = [Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path); $childInfo.UseShellExecute = $false; $childInfo.ArgumentList.Add('-NoProfile'); $childInfo.ArgumentList.Add('-Command'); $childInfo.ArgumentList.Add("[Console]::WriteLine('SIXTH')"); $childProcess = [Diagnostics.Process]::Start($childInfo); $childProcess.WaitForExit(); $childProcess.Dispose(); [Console]::WriteLine('SEVENTH'); [Console]::Error.WriteLine('STDERR_ONLY')"#,
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert_eq!(
        result.stdout.trim().lines().collect::<Vec<_>>(),
        [
            "FIRST", "SECOND", "THIRD", "FOURTH", "FIFTH", "SIXTH", "SEVENTH"
        ],
        "{result:?}"
    );
    assert_eq!(result.stderr.trim(), "STDERR_ONLY", "{result:?}");
}
#[test]
fn background_interpreter_exit_cleans_descendants_without_observation() {
    let _guard = super::matrix::daemon();
    for shell in ["python", "bun"] {
        for code in [0_i32, 37_i32] {
            let cwd = temp_dir("unobserved-exit");
            let child = if shell == "python" {
                "from pathlib import Path; import time; Path('ready').touch(); time.sleep(2); Path('survived').touch()"
            } else {
                "await Bun.write('ready', ''); await Bun.sleep(2000); await Bun.write('survived', '')"
            };
            let encoded = sonic_rs::to_string(child).unwrap();
            let source = if shell == "python" {
                format!(
                    "import subprocess, sys, time; subprocess.Popen([sys.executable, '-c', {encoded}], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL); time.sleep(0.5); sys.exit({code})"
                )
            } else {
                format!(
                    "(await import('node:child_process')).spawn(process.execPath, ['-e', {encoded}], {{stdio:'ignore'}}); await Bun.sleep(500); process.exit({code})"
                )
            };
            let tab = create_tab(&cwd, shell).tab_id;
            let accepted = send_command(&tab, &source, 0.0);
            assert!(!parse_command_result(&accepted).finished, "{accepted:?}");
            let command_id = parse_command_id(&accepted);
            std::thread::sleep(Duration::from_secs(3));
            assert!(cwd.join("ready").exists(), "{shell}: child did not start");
            assert!(
                !cwd.join("survived").exists(),
                "{shell}: descendant outlived the interpreter"
            );
            let finished = parse_command_result(&run_cli(&["view", &command_id]));
            assert_eq!(finished.exit_code, Some(code), "{finished:?}");
            assert!(!parse_tab_view(&run_cli(&["view", &tab])).alive);
        }
    }
}
