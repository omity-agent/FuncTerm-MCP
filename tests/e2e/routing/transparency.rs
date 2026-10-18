use crate::support::{
    create_tab, locked_with_env, manual_write, parse_command_id, parse_command_result, run_cli,
    send_command, temp_dir,
};
use core::time::Duration;
use std::process::Output;
use std::time::Instant;
#[path = "transparency/exit_status.rs"]
mod exit_status;
#[cfg(windows)]
#[path = "transparency/object_format.rs"]
mod object_format;

#[test]
fn terminal_mode_preserves_tty_and_switches_back_to_separated_output() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-handles");
    #[cfg(windows)]
    let shells = ["powershell", "cmd", "bash", "nu", "python", "bun"];
    #[cfg(unix)]
    let shells = ["bash", "zsh", "python", "bun"];
    for shell in shells {
        let tab = create_tab(&directory, shell);
        let query = tty_query(shell);
        let separated = parse_command_result(&send_command(&tab.tab_id, &query, 10.0));
        assert_eq!(
            separated.exit_code,
            Some(0_i32),
            "{shell}: {}",
            separated.stderr
        );
        let direct = run_terminal(&tab.tab_id, &query, "10");
        let text = checked_text(&direct);
        let output = terminal_output(&text);
        assert!(
            output.to_lowercase().contains("tty_state true true true"),
            "{shell}: {text}"
        );
        assert!(output.contains("TERMINAL_ERROR"), "{shell}: {text}");
        assert!(!text.contains("<STDOUT>"), "{shell}: {text}");
        assert!(!text.contains("<STDERR>"), "{shell}: {text}");
        assert!(text.contains("<EXIT_CODE>\n0\n"), "{shell}: {text}");
        let next = parse_command_result(&send_command(&tab.tab_id, &query, 10.0));
        assert_eq!(next.exit_code, Some(0_i32), "{shell}: {}", next.stderr);
        assert!(
            next.stdout.contains("TTY_STATE"),
            "{shell}: {}",
            next.stdout
        );
        assert!(
            next.stderr.contains("TERMINAL_ERROR"),
            "{shell}: {}",
            next.stderr
        );
        let saved = run_cli(&["view", &parse_command_id(&direct)]);
        assert_eq!(terminal_output(&checked_text(&saved)), output, "{shell}");
        assert!(
            run_cli(&["close", "--tab-id", &tab.tab_id])
                .status
                .success()
        );
    }
}

#[test]
fn terminal_transcript_keeps_scrolled_unicode_output_and_command_boundaries() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-transcript");
    let tab = create_tab(&directory, parent_shell());
    let prior = send_command(&tab.tab_id, "echo BEFORE_CAPTURE", 10.0);
    assert!(prior.status.success());
    let command = "python -c \"import sys; [print('记录_%03d' % i, flush=True) for i in range(150)]; print('FINAL_ERROR', file=sys.stderr, flush=True); sys.exit(7)\"";
    let result = run_terminal(&tab.tab_id, command, "10");
    let text = checked_text(&result);
    let output = terminal_output(&text);
    assert!(output.contains("记录_000"), "{text}");
    assert!(output.contains("记录_149"), "{text}");
    assert!(output.contains("FINAL_ERROR"), "{text}");
    assert!(!output.contains("BEFORE_CAPTURE"), "{text}");
    assert!(!output.contains("FuncTerm;"), "{text}");
    assert!(text.contains("<EXIT_CODE>\n7\n"), "{text}");
    let saved_id = parse_command_id(&result);
    let next = run_terminal(&tab.tab_id, "echo AFTER_CAPTURE", "10");
    assert!(terminal_output(&checked_text(&next)).contains("AFTER_CAPTURE"));
    let saved = checked_text(&run_cli(&["view", &saved_id]));
    assert_eq!(terminal_output(&saved), output);
    assert!(!saved.contains("AFTER_CAPTURE"), "{saved}");
}

#[test]
fn terminal_mode_accepts_input_and_honors_user_redirection() {
    let _guard = locked_with_env(&[]);
    let directory = temp_dir("terminal-interaction");
    let tab = create_tab(&directory, parent_shell());
    let command =
        "python -c \"reply = input('INPUT_READY:'); print('INPUT_RECEIVED_' + reply, flush=True)\"";
    let pending = run_terminal(&tab.tab_id, command, "0.3");
    let id = parse_command_id(&pending);
    let mut pending_text = checked_text(&pending);
    let started = Instant::now();
    while !terminal_output(&pending_text).contains("INPUT_READY:") {
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "{pending_text}"
        );
        pending_text = checked_text(&run_cli(&["view", &id, "--wait-timeout", "0.05"]));
    }
    assert!(
        pending_text.contains("<FINISHED>\nfalse\n"),
        "{pending_text}"
    );
    let input = manual_write(&tab.tab_id, b"hello\r", 0.1);
    assert!(input.status.success(), "{input:?}");
    let finished = checked_text(&run_cli(&["view", &id, "--wait-timeout", "10"]));
    assert!(
        terminal_output(&finished).contains("INPUT_RECEIVED_hello"),
        "{finished}"
    );
    assert!(finished.contains("<FINISHED>\ntrue\n"), "{finished}");
    let redirected = run_terminal(
        &tab.tab_id,
        "python -c \"import sys; print('FILE_ONLY'); print('SCREEN_ONLY', file=sys.stderr)\" > user-output.txt",
        "10",
    );
    let redirected_text = checked_text(&redirected);
    let output = terminal_output(&redirected_text);
    assert!(output.contains("SCREEN_ONLY"), "{redirected_text}");
    assert!(!output.contains("FILE_ONLY"), "{redirected_text}");
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

fn checked_text(result: &Output) -> String {
    assert!(result.status.success(), "{result:?}");
    String::from_utf8(result.stdout.clone()).unwrap()
}

fn terminal_output(text: &str) -> &str {
    text.split_once("<OUTPUT>\n")
        .unwrap()
        .1
        .split_once("</OUTPUT>")
        .unwrap()
        .0
}

const fn parent_shell() -> &'static str {
    if cfg!(windows) { "powershell" } else { "bash" }
}

fn tty_query(shell: &str) -> String {
    match shell {
        "bun" => "console.log('TTY_STATE', Boolean(process.stdin.isTTY), Boolean(process.stdout.isTTY), Boolean(process.stderr.isTTY)); console.error('TERMINAL_ERROR')".to_owned(),
        "python" => "import sys; print('TTY_STATE', *(stream.isatty() for stream in (sys.stdin, sys.stdout, sys.stderr))); print('TERMINAL_ERROR', file=sys.stderr)".to_owned(),
        _ => "python -c \"import sys; print('TTY_STATE', *(stream.isatty() for stream in (sys.stdin, sys.stdout, sys.stderr))); print('TERMINAL_ERROR', file=sys.stderr)\"".to_owned(),
    }
}
