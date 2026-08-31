use super::{CONTINUATION_FLAG, FrameReader, FrameWriter};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
struct Message {
    value: String,
}
#[test]
fn large_values_stream_across_continuation_chunks() {
    let expected = Message {
        value: "streamed-value-".repeat(20),
    };
    let mut wire = Vec::new();
    let mut writer = FrameWriter::<_, 16>::new(&mut wire);
    let buffered = sonic_rs::writer::BufferedWriter::new(&mut writer);
    sonic_rs::to_writer(buffered, &expected).unwrap();
    writer.finish().unwrap();
    assert!(continuation_count(&wire) > 1);
    let mut cursor = Cursor::new(&wire);
    let mut reader = FrameReader::open(&mut cursor).unwrap().unwrap();
    let actual: Message = sonic_rs::from_reader(&mut reader).unwrap();
    assert_eq!(actual, expected);
    assert_eq!(usize::try_from(cursor.position()).unwrap(), wire.len());
}
#[test]
fn small_values_keep_the_single_frame_wire_format() {
    let mut wire = Vec::new();
    let mut writer = FrameWriter::<_, 16>::new(&mut wire);
    let buffered = sonic_rs::writer::BufferedWriter::new(&mut writer);
    sonic_rs::to_writer(buffered, &"ok").unwrap();
    writer.finish().unwrap();
    let (Some(header), Some(body)) = (wire.get(..4), wire.get(4..)) else {
        panic!("single-frame message is incomplete");
    };
    assert_eq!(header, &4_u32.to_be_bytes());
    assert_eq!(body, br#""ok""#);
}
fn continuation_count(wire: &[u8]) -> usize {
    let mut offset = 0_usize;
    let mut chunks = 0_usize;
    loop {
        let header_end = offset.saturating_add(4);
        let Some(header_slice) = wire.get(offset..header_end) else {
            panic!("continuation header is incomplete");
        };
        let Ok(header_bytes) = <[u8; 4]>::try_from(header_slice) else {
            panic!("continuation header has an invalid length");
        };
        let header = u32::from_be_bytes(header_bytes);
        offset = header_end;
        offset = offset.saturating_add(usize::try_from(header & !CONTINUATION_FLAG).unwrap());
        chunks = chunks.saturating_add(1);
        if header & CONTINUATION_FLAG == 0 {
            assert_eq!(offset, wire.len());
            return chunks;
        }
    }
}
