use super::{create_record, wait_for_done};
use core::time::Duration;
use std::path::Path;
#[test]
fn zero_wait_does_not_block_for_missing_done_file() {
    let missing_path = Path::new("Z:\\definitely-missing-command.done");
    assert!(!wait_for_done(missing_path, Duration::from_millis(0)).unwrap());
}
#[test]
fn command_record_places_payload_next_to_output_files() {
    let root = std::env::temp_dir()
        .join("functerm-record-payload-test")
        .join(std::process::id().to_string());
    let _ignored = std::fs::remove_dir_all(&root);
    let record = create_record(&root, "command-test", Path::new("F:\\cwd")).unwrap();
    assert_eq!(
        record.payload,
        root.join("command-test").join("command.b64")
    );
    assert_eq!(
        record.stdout.parent().unwrap(),
        record.payload.parent().unwrap()
    );
    std::fs::remove_dir_all(&root).unwrap();
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
