use super::wait_for_done;
use core::time::Duration;
use std::path::Path;
#[test]
fn zero_wait_does_not_block_for_missing_done_file() {
    let missing_path = Path::new("Z:\\definitely-missing-command.done");
    assert!(!wait_for_done(missing_path, Duration::from_millis(0)).unwrap());
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
#[cfg(windows)]
#[test]
fn reads_done_file_after_transient_windows_lock() {
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;
    use std::os::windows::fs::OpenOptionsExt as _;
    use std::sync::mpsc;
    use std::thread;
    let path =
        std::env::temp_dir().join(format!("shell-mcp-locked-done-{}.json", nanoid::nanoid!()));
    let path_for_thread = path.clone();
    let (ready_tx, ready_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .share_mode(0)
            .open(&path_for_thread)
            .unwrap();
        file.write_all(br#"{"exit_code":0,"cwd":"F:\\workspace"}"#)
            .unwrap();
        file.flush().unwrap();
        ready_tx.send(()).unwrap();
        thread::sleep(Duration::from_millis(200));
    });
    ready_rx.recv().unwrap();
    let done = super::read_done(&path).unwrap().unwrap();
    writer.join().unwrap();
    assert_eq!(done.exit_code, 0_i32);
    assert_eq!(done.cwd, "F:\\workspace");
    fs::remove_file(path).unwrap();
}
