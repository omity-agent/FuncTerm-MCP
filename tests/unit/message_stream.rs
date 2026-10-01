use serde::{Deserialize, Serialize};
use std::io::{self, BufReader, Cursor};
#[test]
fn incomplete_messages_are_errors_not_clean_eof() {
    assert!(
        super::read_or_eof::<String, _>(&mut Cursor::new([]))
            .unwrap()
            .is_none()
    );
    let mut wire = Vec::new();
    super::write(&mut wire, &"value").unwrap();
    for end in 1..wire.len() {
        let (prefix, _) = wire.split_at(end);
        drop(super::read_or_eof::<String, _>(&mut Cursor::new(prefix)).unwrap_err());
    }
}
#[test]
fn consecutive_messages_do_not_consume_each_other() {
    let mut wire = Vec::new();
    super::write(&mut wire, &"first").unwrap();
    super::write(&mut wire, &"second").unwrap();
    let mut reader = BufReader::new(Cursor::new(wire));
    assert_eq!(
        super::read_or_eof::<String, _>(&mut reader)
            .unwrap()
            .as_deref(),
        Some("first")
    );
    assert_eq!(
        super::read_or_eof::<String, _>(&mut reader)
            .unwrap()
            .as_deref(),
        Some("second")
    );
    assert!(
        super::read_or_eof::<String, _>(&mut reader)
            .unwrap()
            .is_none()
    );
}
#[test]
fn writer_handles_short_writes_and_empty_values() {
    for length in [0, 1, 15, 16, 17, 32, 33, 8192, 8193] {
        let payload = "x".repeat(length);
        let mut wire = Vec::new();
        super::write(&mut ShortWriter(&mut wire), &payload).unwrap();
        let actual: String = super::read_or_eof(&mut Cursor::new(wire)).unwrap().unwrap();
        assert_eq!(actual, payload);
    }
}
#[test]
fn invalid_message_is_not_clean_eof() {
    drop(super::read_or_eof::<String, _>(&mut Cursor::new([0xc1])).unwrap_err());
}
#[test]
fn reads_retry_interrupted_and_short_reads() {
    let mut wire = Vec::new();
    super::write(&mut wire, &"ok").unwrap();
    let mut reader = BufReader::new(FragmentedReader {
        bytes: Cursor::new(wire),
        calls: 0,
    });
    assert_eq!(
        super::read_or_eof::<String, _>(&mut reader)
            .unwrap()
            .as_deref(),
        Some("ok")
    );
}
struct ShortWriter<'buffer>(&'buffer mut Vec<u8>);
impl io::Write for ShortWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let amount = buf.len().min(1);
        let (portion, _) = buf.split_at(amount);
        self.0.extend_from_slice(portion);
        Ok(amount)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
struct FragmentedReader {
    bytes: Cursor<Vec<u8>>,
    calls: usize,
}
impl io::Read for FragmentedReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.calls += 1;
        if matches!(self.calls, 1 | 3 | 5) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = buf.len().min(1);
        let (portion, _) = buf.split_at_mut(count);
        self.bytes.read(portion)
    }
}
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
struct Message {
    value: String,
}
#[test]
fn large_values_stream_without_custom_chunks() {
    let expected = Message {
        value: "streamed-value-中文🦀\n".repeat(100_000),
    };
    let mut wire = Vec::new();
    super::write(&mut wire, &expected).unwrap();
    let mut reader = BufReader::new(Cursor::new(wire));
    let actual: Message = super::read_or_eof(&mut reader).unwrap().unwrap();
    assert_eq!(actual, expected);
    assert!(
        super::read_or_eof::<Message, _>(&mut reader)
            .unwrap()
            .is_none()
    );
}
#[test]
fn small_values_use_standard_messagepack() {
    let mut wire = Vec::new();
    super::write(&mut wire, &"ok").unwrap();
    assert_eq!(wire, [0xa2, b'o', b'k']);
}
