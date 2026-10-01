use super::{create_record, read_and_clear_command_result, read_command_result};
use crate::runtime::session::wait_for_path;
use crate::shell::ShellChoice;
use core::time::Duration;
use std::path::Path;
#[test]
fn zero_wait_does_not_block_for_missing_done_file() {
    let missing_path = Path::new("Z:\\definitely-missing-command.done");
    assert!(!wait_for_path(missing_path, Duration::from_millis(0)).unwrap());
}
#[test]
fn command_record_separates_input_output_and_state_files() {
    let root = crate::test_fs::temp_dir("record-payload");
    let _cleanup = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-test", Path::new("F:\\cwd")).unwrap();
    assert_eq!(
        record.command,
        root.join("command-test").join("input").join("command.txt")
    );
    assert_eq!(
        record.stdout,
        root.join("command-test").join("output").join("stdout.txt")
    );
    assert_eq!(
        record.script_for(ShellChoice::Cmd),
        root.join("command-test").join("input").join("command.cmd")
    );
    assert_eq!(
        record.script_for(ShellChoice::PowerShell),
        root.join("command-test").join("input").join("command.ps1")
    );
    assert_eq!(
        record.done,
        root.join("command-test").join("state").join("done.json")
    );
    assert_eq!(
        std::fs::read_to_string(root.join("command-test").join("input").join("cwd.txt")).unwrap(),
        "F:\\cwd"
    );
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn published_result_closes_command_lifecycle() {
    let root = crate::test_fs::temp_dir("record-failed-result");
    let _final_cleanup = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-failed", Path::new("F:\\cwd")).unwrap();
    std::fs::write(
        &record.done,
        r#"{"exit_code":1,"time_consumption":"1ms","cwd":"F:/cwd"}"#,
    )
    .unwrap();
    assert!(wait_for_path(&record.done, Duration::from_millis(0)).unwrap());
    let result =
        read_command_result(&record, Duration::from_millis(1), "FuncTerm".to_owned()).unwrap();
    assert!(result.command.finished);
    assert_eq!(result.command.exit_code, Some(1_i32));
    assert!(
        result
            .note
            .contains("No stdout or stderr content was captured.")
    );
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn read_and_clear_keeps_result_while_removing_record_files() {
    let root = crate::test_fs::temp_dir("record-clear");
    let _ignored = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-clear", Path::new("F:\\cwd")).unwrap();
    std::fs::write(&record.stdout, "\x1b[32mkept stdout\x1b[0m").unwrap();
    std::fs::write(&record.stderr, "\x1b[31mkept stderr\x1b[0m").unwrap();
    std::fs::write(
        &record.done,
        r#"{"exit_code":0,"time_consumption":"1ms","cwd":"F:/cwd"}"#,
    )
    .unwrap();
    let result =
        read_and_clear_command_result(&record, Duration::from_millis(1), "FuncTerm".to_owned())
            .unwrap();
    assert!(result.command.finished);
    assert_eq!(result.command.stdout, "kept stdout");
    assert_eq!(result.command.stderr, "kept stderr");
    assert!(!record.directory.exists());
    let _clear_cleanup = std::fs::remove_dir_all(&root);
}
#[test]
fn reads_utf16_little_endian_output() {
    let bytes = [
        0xFF, 0xFE, b'H', 0x00, b'E', 0x00, b'L', 0x00, b'L', 0x00, b'O', 0x00,
    ];
    let text = super::decode_text(&bytes).unwrap();
    assert_eq!(text, "HELLO");
}
#[test]
fn reads_utf8_with_bom_output() {
    let text = super::decode_text(&[0xEF, 0xBB, 0xBF, b'{', b'}']).unwrap();
    assert_eq!(text, "{}");
}
#[test]
fn command_output_strips_terminal_sequences_without_changing_text_or_files() {
    let root = crate::test_fs::temp_dir("ansi-output");
    let record = create_record(&root, "command-ansi", &root).unwrap();
    for (input, expected) in [
        (
            "plain 中文 🦀\ttext\r\nnext\n",
            "plain 中文 🦀\ttext\r\nnext\n",
        ),
        ("\x1b[1;31mred\x1b[0m", "red"),
        ("\x1b[38;5;196m中文\x1b[m", "中文"),
        ("\x1b[38;2;255;0;0m🦀\x1b[0m", "🦀"),
        ("\x1b[48:2::0:128:255m背景\x1b[m", "背景"),
        ("\x1b[2J\x1b[Htext\x1b[?25l\x1b[?25h", "text"),
        ("\x1b]0;title\x07text", "text"),
        (
            "\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\",
            "link",
        ),
        ("\x1bP1;2qhidden\x1b\\text", "text"),
        (
            "\x1b_hidden\x1b\\\x1b^hidden\x1b\\\x1bXhidden\x1b\\text",
            "text",
        ),
        ("before\x1b[38;2;", "before"),
        ("before\x1b[38;2;255;0;0mafter\x1b[0m", "beforeafter"),
        ("before\x1b]0;unfinished", "before"),
        ("\x1b[31m\x1b[0m", ""),
    ] {
        std::fs::write(&record.stdout, input).unwrap();
        std::fs::write(&record.stderr, input).unwrap();
        let result = read_command_result(&record, Duration::ZERO, "FuncTerm".to_owned()).unwrap();
        assert_eq!(result.command.stdout, expected, "{input:?}");
        assert_eq!(result.command.stderr, expected, "{input:?}");
        assert!(!result.command.finished);
        assert_eq!(
            result
                .note
                .contains("No stdout or stderr content was captured."),
            expected.is_empty()
        );
        assert_eq!(std::fs::read_to_string(&record.stdout).unwrap(), input);
        assert_eq!(std::fs::read_to_string(&record.stderr).unwrap(), input);
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn output_filter_runs_after_decoding_utf16_and_utf8_bom() {
    let root = crate::test_fs::temp_dir("encoded-ansi");
    let path = root.join("captured.txt");
    let input = "\x1b[31m中文 🦀\ttext\r\n\x1b[0m";
    for little_endian in [true, false] {
        let mut bytes = if little_endian {
            vec![0xFF, 0xFE]
        } else {
            vec![0xFE, 0xFF]
        };
        for unit in input.encode_utf16() {
            #[expect(
                clippy::little_endian_bytes,
                reason = "UTF-16LE fixtures require little-endian bytes on every host"
            )]
            bytes.extend_from_slice(&if little_endian {
                unit.to_le_bytes()
            } else {
                unit.to_be_bytes()
            });
        }
        std::fs::write(&path, bytes).unwrap();
        assert_eq!(
            super::read_plain_output(&path).unwrap(),
            "中文 🦀\ttext\r\n"
        );
    }
    std::fs::write(&path, format!("\u{feff}{input}")).unwrap();
    assert_eq!(
        super::read_plain_output(&path).unwrap(),
        "中文 🦀\ttext\r\n"
    );
    std::fs::remove_dir_all(root).unwrap();
}
