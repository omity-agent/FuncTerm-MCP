use super::create_parser;
use tastty_core::TerminalSize;
const SIZE: TerminalSize = TerminalSize {
    rows: 30,
    cols: 120,
};
#[test]
fn startup_title_changes_do_not_replace_initial_title() {
    let mut terminal = create_parser(SIZE, 0, "FuncTerm").unwrap();
    terminal.process(b"\x1b]2;Shell startup title\x1b\\");
    assert_eq!(terminal.title(), "FuncTerm");
    terminal.process(b"\x1b]133;B\x1b\\");
    assert_eq!(terminal.title(), "FuncTerm");
}
#[test]
fn title_changes_after_first_prompt_replace_initial_title() {
    let mut terminal = create_parser(SIZE, 0, "FuncTerm").unwrap();
    terminal.process(b"\x1b]2;Shell startup title\x1b\\\x1b]133;B\x1b\\");
    terminal.process(b"\x1b]2;Command title\x1b\\");
    assert_eq!(terminal.title(), "Command title");
}
#[test]
fn prompt_boundary_preserves_event_order_within_one_chunk() {
    let mut terminal = create_parser(SIZE, 0, "FuncTerm").unwrap();
    terminal.process(b"\x1b]2;Shell startup title\x1b\\\x1b]133;B\x1b\\\x1b]2;Command title\x1b\\");
    assert_eq!(terminal.title(), "Command title");
}
#[test]
fn control_characters_are_rejected() {
    let result = create_parser(SIZE, 0, "unsafe\x1btitle");
    let Err(error) = result else {
        panic!("control characters should be rejected");
    };
    assert_eq!(
        error.to_string(),
        "terminal_initial_title must not contain control characters"
    );
}
