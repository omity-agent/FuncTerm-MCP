use crate::support::{
    complete_command, create_tab, locked_with_env, manual_write, parse_command_id,
    parse_command_result, parse_tab_view, run_cli, send_command, tagged_content, temp_dir,
    wait_for_completion,
};
use core::time::Duration;
use std::process::Output;
use std::time::Instant;
#[test]
fn terminal_mode_preserves_tty_and_switches_back_to_separated_output() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-handles");
    #[cfg(windows)]
    let shells = ["powershell", "cmd", "bash", "nu", "python", "bun"];
    #[cfg(unix)]
    let shells = ["bash", "zsh", "python", "bun"];
    for shell in shells {
        let tab = create_tab(&directory, shell).tab_id;
        let query = tty_query(shell);
        let separated = complete_command(&tab, query);
        assert_eq!(separated.exit_code, Some(0_i32), "{shell}: {separated:?}");
        let direct = run_terminal(&tab, query, "10");
        let terminal = wait_for_completion(&direct);
        let document = String::from_utf8(direct.stdout.clone()).unwrap();
        for tag in ["STDOUT", "STDERR"] {
            assert!(tagged_content(&document, tag).is_none(), "{document}");
        }
        let native = if shell == "bun" { "false" } else { "true" };
        let expected = format!("tty_state true {native} {native}");
        assert!(
            terminal.stdout.to_lowercase().contains(&expected),
            "{terminal:?}"
        );
        assert!(terminal.stdout.contains("TERMINAL_ERROR"), "{terminal:?}");
        assert_eq!(terminal.exit_code, Some(0_i32), "{terminal:?}");
        let next = complete_command(&tab, query);
        assert_eq!(next.exit_code, Some(0_i32), "{shell}: {next:?}");
        assert!(next.stdout.contains("TTY_STATE"), "{shell}: {next:?}");
        let errors = if shell == "nu" {
            &next.stdout
        } else {
            &next.stderr
        };
        assert!(errors.contains("TERMINAL_ERROR"), "{shell}: {next:?}");
        let saved = parse_command_result(&run_cli(&["view", &parse_command_id(&direct)]));
        assert_eq!(saved.stdout, terminal.stdout, "{shell}");
        assert!(run_cli(&["close", "--tab-id", &tab]).status.success());
    }
}
#[test]
fn terminal_state_keeps_scrolled_unicode_output_and_command_boundaries() {
    let _guard = locked_with_env(&[]);
    let tab = create_tab(&temp_dir("rendered-unicode"), parent_shell()).tab_id;
    assert!(
        send_command(&tab, "echo BEFORE_CAPTURE", 10.0)
            .status
            .success()
    );
    let command = "python -c \"import sys; sys.stdout.write(''.join('记录_%04d\\n' % i for i in range(4200))); print('FINAL_ERROR', file=sys.stderr); sys.exit(7)\"";
    let accepted = run_terminal(&tab, command, "10");
    let result = wait_for_completion(&accepted);
    for expected in ["记录_0000", "记录_4199", "FINAL_ERROR"] {
        assert!(result.stdout.contains(expected), "{result:?}");
    }
    for excluded in ["BEFORE_CAPTURE", "FuncTerm;"] {
        assert!(!result.stdout.contains(excluded), "{result:?}");
    }
    assert_eq!(result.exit_code, Some(7_i32), "{result:?}");
    let screen = parse_tab_view(&run_cli(&["view", &tab])).screen;
    assert!(!screen.contains("记录_0000"), "{screen}");
    assert!(screen.contains("记录_4199"), "{screen}");
    let next = wait_for_completion(&run_terminal(&tab, "echo AFTER_CAPTURE", "10"));
    assert!(next.stdout.contains("AFTER_CAPTURE"), "{next:?}");
    let saved = parse_command_result(&run_cli(&["view", &parse_command_id(&accepted)]));
    assert_eq!(saved.stdout, result.stdout);
    assert!(!saved.stdout.contains("AFTER_CAPTURE"), "{saved:?}");
}
#[test]
fn terminal_mode_accepts_input_and_honors_user_redirection() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-interaction");
    let tab = create_tab(&directory, parent_shell()).tab_id;
    let command =
        "python -c \"reply = input('INPUT_READY:'); print('INPUT_RECEIVED_' + reply, flush=True)\"";
    let accepted = run_terminal(&tab, command, "0.3");
    let id = parse_command_id(&accepted);
    let mut pending = parse_command_result(&accepted);
    let started = Instant::now();
    while !pending.stdout.contains("INPUT_READY:") {
        assert!(started.elapsed() < Duration::from_secs(10), "{pending:?}");
        pending = parse_command_result(&run_cli(&["view", &id, "--wait-timeout", "0.05"]));
    }
    assert!(!pending.finished, "{pending:?}");
    assert!(manual_write(&tab, b"hello\r", 0.1).status.success());
    let finished = parse_command_result(&run_cli(&["view", &id, "--wait-timeout", "10"]));
    assert!(
        finished.stdout.contains("INPUT_RECEIVED_hello"),
        "{finished:?}"
    );
    assert!(finished.finished, "{finished:?}");
    let redirected = run_terminal(
        &tab,
        "python -c \"import sys; print('FILE_ONLY'); print('SCREEN_ONLY', file=sys.stderr)\" > user-output.txt",
        "10",
    );
    let result = parse_command_result(&redirected);
    assert!(result.stdout.contains("SCREEN_ONLY"), "{result:?}");
    assert!(!result.stdout.contains("FILE_ONLY"), "{result:?}");
    assert!(
        fs_err::read_to_string(directory.join("user-output.txt"))
            .unwrap()
            .contains("FILE_ONLY")
    );
}
fn run_terminal(tab_id: &str, command: &str, wait_timeout: &str) -> Output {
    run_cli(&[
        "send-command",
        tab_id,
        "--command",
        command,
        "--separate-output",
        "false",
        "--wait-timeout",
        wait_timeout,
    ])
}
const fn parent_shell() -> &'static str {
    if cfg!(windows) { "powershell" } else { "bash" }
}
const fn tty_query(shell: &str) -> &'static str {
    match shell.as_bytes() {
        b"bun" => {
            "console.log('TTY_STATE', Boolean(process.stdin.isTTY), Boolean(process.stdout.isTTY), Boolean(process.stderr.isTTY)); console.error('TERMINAL_ERROR')"
        }
        b"python" => {
            "import sys; print('TTY_STATE', *(stream.isatty() for stream in (sys.stdin, sys.stdout, sys.stderr))); print('TERMINAL_ERROR', file=sys.stderr)"
        }
        _ => {
            "python -c \"import sys; print('TTY_STATE', *(stream.isatty() for stream in (sys.stdin, sys.stdout, sys.stderr))); print('TERMINAL_ERROR', file=sys.stderr)\""
        }
    }
}
