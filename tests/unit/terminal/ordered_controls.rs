use super::Terminal;
use tastty_core::{HostProfile, TerminalSize};
fn terminal() -> Terminal {
    Terminal::new(
        TerminalSize {
            rows: 30,
            cols: 120,
        },
        0,
        "FuncTerm",
    )
    .unwrap()
}
#[test]
fn interleaved_title_markers_and_mode_changes_preserve_reply_order() {
    let output = concat!(
        "\x1b[6n",
        "\x1b[?9001h",
        "\x1b]9999;FuncTerm;start;command-mode\x1b\\",
        "\x1b]2;模式与标题\x07",
        "\x1b[6n",
        "\x1b[?9001l",
        "\x1b[6n",
        "\x1b]9999;FuncTerm;end;command-mode\x1b\\",
        "\x1b]2;Shell\x07"
    )
    .as_bytes();
    for chunk_size in 1..=output.len() {
        let screen = terminal();
        let title = screen.capture_title("command-mode").unwrap();
        let replies = output
            .chunks(chunk_size)
            .flat_map(|chunk| screen.process(chunk, &HostProfile::default()).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            replies
                .iter()
                .map(|reply| reply.win32_input)
                .collect::<Vec<_>>(),
            [false, true, false]
        );
        assert!(replies.iter().all(|reply| reply.bytes == b"\x1b[1;1R"));
        assert_eq!(title.wait_finished().unwrap(), "模式与标题");
        assert_eq!(screen.raw_title(), "Shell");
    }
}
#[test]
fn only_private_non_subparameter_9001_changes_input_encoding() {
    let screen = terminal();
    let output = b"\x1b[9001h\x1b[6n\x1b[?9001:1h\x1b[6n\x1b[?1;9001h\x1b[6n\x1b[?9001l\x1b[6n";
    let replies = screen.process(output, &HostProfile::default()).unwrap();
    assert_eq!(
        replies
            .iter()
            .map(|reply| reply.win32_input)
            .collect::<Vec<_>>(),
        [false, false, true, false]
    );
}
#[test]
fn invalid_markers_fail_registered_captures_and_reader() {
    for output in [
        b"\x1b]9999;FuncTerm;unknown;command-invalid\x07".as_slice(),
        b"\x1b]9999;FuncTerm;start;\xff\x07",
    ] {
        let screen = terminal();
        let title = screen.capture_title("command-invalid").unwrap();
        assert!(screen.process(output, &HostProfile::default()).is_err());
        drop(title.wait_finished().unwrap_err());
        drop(screen.output_revision().unwrap_err());
    }
}
#[test]
fn closed_reader_rejects_new_command_capture() {
    let closed = terminal();
    closed.reader_closed();
    assert!(closed.capture_title("after-close").is_err());
    let failed = terminal();
    failed.reader_failed("test reader failure");
    assert!(failed.capture_title("after-failure").is_err());
}
