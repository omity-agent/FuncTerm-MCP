use super::{
    create_record, read_and_clear_command_result, read_command_result, wait_for_done,
    write_failed_result,
};
use core::time::Duration;
use std::path::Path;
#[test]
fn zero_wait_does_not_block_for_missing_done_file() {
    let missing_path = Path::new("Z:\\definitely-missing-command.done");
    assert!(!wait_for_done(missing_path, Duration::from_millis(0)).unwrap());
}
#[test]
fn command_record_separates_input_output_and_state_files() {
    let root = std::env::temp_dir()
        .join("functerm-record-payload-test")
        .join(std::process::id().to_string());
    let _cleanup = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-test", Path::new("F:\\cwd")).unwrap();
    assert_eq!(
        record.payload,
        root.join("command-test").join("input").join("command.b64")
    );
    assert_eq!(
        record.stdout,
        root.join("command-test").join("output").join("stdout.txt")
    );
    assert_eq!(
        record.done,
        root.join("command-test").join("state").join("done.json")
    );
    std::fs::remove_dir_all(&root).unwrap();
}
#[test]
fn failed_result_closes_command_lifecycle() {
    let root = std::env::temp_dir()
        .join("functerm-record-failed-result-test")
        .join(std::process::id().to_string());
    let _final_cleanup = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-failed", Path::new("F:\\cwd")).unwrap();
    write_failed_result("command-failed", &record, "shell exited").unwrap();
    assert!(wait_for_done(&record.done, Duration::from_millis(0)).unwrap());
    let result = read_command_result(&record, "1ms".to_owned()).unwrap();
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
    let root = std::env::temp_dir()
        .join("functerm-record-clear-test")
        .join(std::process::id().to_string());
    let _ignored = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-clear", Path::new("F:\\cwd")).unwrap();
    std::fs::write(&record.stdout, "kept stdout").unwrap();
    std::fs::write(&record.stderr, "kept stderr").unwrap();
    write_failed_result("command-clear", &record, "done").unwrap();
    let result = read_and_clear_command_result(&record, "1ms".to_owned()).unwrap();
    assert!(result.command.finished);
    assert_eq!(result.command.stdout, "kept stdout");
    assert!(result.command.stderr.contains("kept stderr"));
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
