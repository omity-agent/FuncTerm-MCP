use crate::support::{
    complete_command, create_tab, manual_write, parse_command_id, parse_command_result,
    parse_tab_view, run_cli, send_command, temp_dir,
};
#[test]
fn python_keeps_future_flags_and_native_standard_streams() {
    let _guard = super::matrix::daemon();
    let tab = create_tab(&temp_dir("compiler"), "python").tab_id;
    let future = complete_command(&tab, "from __future__ import annotations");
    assert_eq!(future.exit_code, Some(0_i32), "{future:?}");
    let current = parse_tab_view(&run_cli(&["view", &tab]));
    assert!(
        current.alive,
        "Python ended after compiling future import: {}",
        current.screen
    );
    let result = complete_command(
        &tab,
        "def f(x: Missing): pass\nimport os, subprocess, sys\nprint(f.__annotations__)\nos.write(1, b'NATIVE_OUT\\n')\nos.write(2, b'NATIVE_ERR\\n')\nsubprocess.run([sys.executable, '-c', \"print('CHILD_OUT')\"])",
    );
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    for expected in ["Missing", "NATIVE_OUT", "CHILD_OUT"] {
        assert!(result.stdout.contains(expected), "{result:?}");
    }
    assert!(result.stderr.contains("NATIVE_ERR"), "{result:?}");
}
#[test]
fn bun_capture_reaches_console_and_native_writes() {
    let _guard = super::matrix::daemon();
    let tab = create_tab(&temp_dir("bun"), "bun").tab_id;
    let source = "console.log(typeof console.log('HOST_OUT')); await Bun.write(Bun.stdout, 'NATIVE_OUT\\n'); await Bun.write(Bun.stderr, 'NATIVE_ERR\\n'); (await import('node:child_process')).spawnSync(process.execPath, ['-e', \"console.log('CHILD_OUT'); console.error('CHILD_ERR')\"], {stdio:'inherit'});";
    let result = complete_command(&tab, source);
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    for expected in ["HOST_OUT", "NATIVE_OUT", "undefined", "CHILD_OUT"] {
        assert!(result.stdout.contains(expected), "{result:?}");
    }
    for expected in ["NATIVE_ERR", "CHILD_ERR"] {
        assert!(result.stderr.contains(expected), "{result:?}");
        assert!(!result.stdout.contains(expected), "{result:?}");
    }
}
#[test]
fn nushell_retains_lexical_variables_and_waits_for_input() {
    let _guard = super::matrix::daemon();
    let tab = create_tab(&temp_dir("lexical-scope"), "nu").tab_id;
    let defined = complete_command(&tab, "let x = 1; mut y = 2; let callback = { $x }");
    assert_eq!(defined.exit_code, Some(0_i32), "{defined:?}");
    let query = complete_command(&tab, "$y = $y + (do $callback); print $y");
    assert_eq!(query.exit_code, Some(0_i32), "{query:?}");
    assert!(query.stdout.contains('3'), "{query:?}");
    let started = send_command(&tab, "let answer = input '提示> '; print $answer", 0.5);
    assert!(!parse_command_result(&started).finished, "{started:?}");
    let id = parse_command_id(&started);
    assert!(manual_write(&tab, b"USER_ANSWER\r\n", 0.5).status.success());
    let result = parse_command_result(&run_cli(&["view", &id, "--wait-timeout", "3"]));
    assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
    assert!(result.stdout.contains("USER_ANSWER"), "{result:?}");
}
#[cfg(windows)]
#[test]
fn cmd_lf_unicode_and_interactive_for_preserve_percent_cwd() {
    let _guard = super::matrix::daemon();
    let cwd = temp_dir("percent % cwd");
    let tab = create_tab(&cwd, "cmd").tab_id;
    for (source, expected) in [
        ("echo A\necho 中文", "中文"),
        ("echo A\necho é", "é"),
        ("for %i in (A B) do @echo %i", "B"),
    ] {
        let result = complete_command(&tab, source);
        assert_eq!(result.exit_code, Some(0_i32), "{result:?}");
        assert!(result.stdout.contains(expected), "{result:?}");
        assert!(result.cwd.contains("percent % cwd"), "{result:?}");
    }
}
#[test]
fn interrupted_shells_return_to_a_usable_prompt() {
    let _guard = super::matrix::daemon();
    for (shell, source, after) in [
        ("bash", "sleep 30", "printf 'AFTER_INTERRUPT\\n'"),
        ("nu", "sleep 30sec", "print AFTER_INTERRUPT"),
        #[cfg(windows)]
        ("cmd", "ping -n 30 127.0.0.1", "echo AFTER_INTERRUPT"),
    ] {
        let tab = create_tab(&temp_dir(shell), shell).tab_id;
        let id = parse_command_id(&send_command(&tab, source, 0.0));
        std::thread::sleep(core::time::Duration::from_millis(600));
        let _interrupted = manual_write(&tab, &[3], 0.5);
        if shell == "cmd" {
            let deadline = std::time::Instant::now() + core::time::Duration::from_secs(3);
            loop {
                let active = parse_command_result(&run_cli(&["view", &id]));
                if active.finished {
                    break;
                }
                let current = parse_tab_view(&run_cli(&["view", &tab, "--wait-timeout", "0.1"]));
                if current.screen.contains("Terminate batch job")
                    || active.stdout.contains("Terminate batch job")
                {
                    let _confirmed = manual_write(&tab, b"Y\r", 0.2);
                }
                if std::time::Instant::now() >= deadline {
                    break;
                }
            }
        }
        let result = parse_command_result(&run_cli(&["view", &id, "--wait-timeout", "3"]));
        assert!(result.finished, "{shell}: {result:?}");
        assert_eq!(result.exit_code, Some(130_i32), "{shell}: {result:?}");
        assert!(
            !matches!(result.time_consumption.as_str(), "0ns" | "0.00ms"),
            "{shell}: {result:?}"
        );
        assert!(
            parse_command_result(&send_command(&tab, after, 5.0))
                .stdout
                .contains("AFTER_INTERRUPT"),
            "{shell}"
        );
    }
}
