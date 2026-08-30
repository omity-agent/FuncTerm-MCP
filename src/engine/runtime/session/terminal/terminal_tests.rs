use super::Terminal;
use alloc::sync::Arc;
use core::time::Duration;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use tastty_core::{HostProfile, TerminalSize};
const SIZE: TerminalSize = TerminalSize {
    rows: 30,
    cols: 120,
};
#[test]
fn command_without_title_ignores_titles_outside_its_boundaries() {
    let terminal = Terminal::new(SIZE, 0, "FuncTerm").unwrap();
    let capture = terminal.capture_title("command-a").unwrap();
    process (& terminal , b"\x1b]2;Shell before\x1b\\\x1b]9999;FuncTerm;start;command-a\x1b\\output\x1b]9999;FuncTerm;end;command-a\x1b\\\x1b]2;Shell after\x1b\\" ,) ;
    assert_eq!(capture.wait_finished().unwrap(), "FuncTerm");
    assert_eq!(terminal.raw_title(), "Shell after");
}
#[test]
fn command_title_is_frozen_before_shell_restores_its_title() {
    let bytes = b"\x1b]9999;FuncTerm;start;command-b\x1b\\\x1b]2;Command title\x07\x1b]9999;FuncTerm;end;command-b\x1b\\\x1b]2;Shell title\x07" ;
    for split in 0..=bytes.len() {
        let terminal = Terminal::new(SIZE, 0, "FuncTerm").unwrap();
        let capture = terminal.capture_title("command-b").unwrap();
        let (before_split, after_split) = bytes.split_at(split);
        process(&terminal, before_split);
        process(&terminal, after_split);
        assert_eq!(capture.wait_finished().unwrap(), "Command title");
        assert_eq!(terminal.raw_title(), "Shell title");
    }
}
#[test]
fn repeated_window_title_still_counts_as_command_output() {
    let terminal = Terminal::new(SIZE, 0, "FuncTerm").unwrap();
    let capture = terminal.capture_title("command-c").unwrap();
    process (& terminal , b"\x1b]2;Repeated\x07\x1b]9999;FuncTerm;start;command-c\x1b\\\x1b]2;Repeated\x07\x1b]9999;FuncTerm;end;command-c\x1b\\" ,) ;
    assert_eq!(capture.wait_finished().unwrap(), "Repeated");
}
#[test]
fn restored_shell_title_is_not_a_command_title_assignment() {
    let terminal = Terminal::new(SIZE, 0, "FuncTerm").unwrap();
    process(&terminal, b"\x1b]2;Shell title\x07\x1b[22;2t");
    let capture = terminal.capture_title("command-d").unwrap();
    process(
        &terminal,
        b"\x1b]9999;FuncTerm;start;command-d\x1b\\\x1b[23;2t\x1b]9999;FuncTerm;end;command-d\x1b\\",
    );
    assert_eq!(capture.wait_finished().unwrap(), "FuncTerm");
}
#[test]
fn control_characters_are_rejected() {
    let result = Terminal::new(SIZE, 0, "unsafe\x1btitle");
    let Err(error) = result else {
        panic!("control characters should be rejected");
    };
    assert_eq!(
        error.to_string(),
        "terminal_model_title must not contain control characters"
    );
}
#[test]
fn input_waits_for_the_next_terminal_output_revision() {
    let terminal = Arc::new(Terminal::new(SIZE, 0, "FuncTerm").unwrap());
    process(&terminal, b"before");
    let revision = terminal.output_revision().unwrap();
    let waiting_terminal = Arc::clone(&terminal);
    let (waiting_tx, waiting_rx) = mpsc::channel();
    let (returned_tx, returned_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        waiting_tx.send(()).unwrap();
        let result = waiting_terminal.wait_for_output(revision, Duration::from_secs(1));
        returned_tx.send(result).unwrap();
    });
    waiting_rx.recv().unwrap();
    assert!(matches!(returned_rx.try_recv(), Err(TryRecvError::Empty)));
    process(&terminal, b" after");
    returned_rx.recv().unwrap().unwrap();
    worker.join().unwrap();
    assert!(terminal.contents().contains("before after"));
}
#[test]
fn normal_reader_close_wakes_output_wait_without_failure() {
    let terminal = Arc::new(Terminal::new(SIZE, 0, "FuncTerm").unwrap());
    let revision = terminal.output_revision().unwrap();
    let waiting_terminal = Arc::clone(&terminal);
    let worker =
        thread::spawn(move || waiting_terminal.wait_for_output(revision, Duration::from_secs(1)));
    terminal.reader_closed();
    worker.join().unwrap().unwrap();
}
#[test]
fn reader_failure_wakes_output_wait_with_error() {
    let terminal = Terminal::new(SIZE, 0, "FuncTerm").unwrap();
    let revision = terminal.output_revision().unwrap();
    terminal.reader_failed("reader test failure");
    let error = terminal
        .wait_for_output(revision, Duration::from_secs(1))
        .unwrap_err();
    assert!(error.to_string().contains("reader test failure"));
}
#[cfg(windows)]
#[test]
fn startup_replies_stay_raw_before_conpty_win32_input_mode() {
    let output = b"\x1b[6n\x1b[c\x1b[?1004h\x1b[?9001h\x1b[6n";
    for split in 0..=output.len() {
        let terminal = Terminal::new(SIZE, 0, "FuncTerm").unwrap();
        let (before, after) = output.split_at(split);
        let mut replies = terminal.process(before, &HostProfile::default()).unwrap();
        replies.extend(terminal.process(after, &HostProfile::default()).unwrap());
        let mut reply_iterator = replies.into_iter();
        let Some(startup_position_reply) = reply_iterator.next() else {
            panic!("expected a startup cursor position reply");
        };
        let Some(startup_attributes_reply) = reply_iterator.next() else {
            panic!("expected a startup device attributes reply");
        };
        let Some(application_reply) = reply_iterator.next() else {
            panic!("expected an application cursor position reply");
        };
        assert!(reply_iterator.next().is_none());
        assert!(!startup_position_reply.win32_input);
        assert_eq!(startup_position_reply.bytes, b"\x1b[1;1R");
        assert!(!startup_attributes_reply.win32_input);
        assert_eq!(startup_attributes_reply.bytes, b"\x1b[?62;22;52c");
        assert!(application_reply.win32_input);
        assert_eq!(application_reply.bytes, b"\x1b[1;1R");
        let encoded = crate::runtime::session::keyboard::host_reply_bytes(
            &application_reply.bytes,
            application_reply.win32_input,
        )
        .unwrap();
        assert!(encoded.starts_with(b"\x1b[0;0;27;1;0;1_"));
        assert!(encoded.ends_with(b"\x1b[0;0;82;1;0;1_"));
    }
}
fn process(terminal: &Terminal, bytes: &[u8]) {
    terminal.process(bytes, &HostProfile::default()).unwrap();
}
