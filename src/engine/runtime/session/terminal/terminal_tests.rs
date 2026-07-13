use super::Terminal;
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
    assert_eq!(terminal.title().unwrap(), "Shell after");
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
        assert_eq!(terminal.title().unwrap(), "Shell title");
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
fn control_characters_are_rejected() {
    let result = Terminal::new(SIZE, 0, "unsafe\x1btitle");
    let Err(error) = result else {
        panic!("control characters should be rejected");
    };
    assert_eq!(
        error.to_string(),
        "terminal_initial_title must not contain control characters"
    );
}
fn process(terminal: &Terminal, bytes: &[u8]) {
    terminal.process(bytes, &HostProfile::default()).unwrap();
}
