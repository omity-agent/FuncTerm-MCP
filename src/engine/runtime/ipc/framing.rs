use anyhow::{Context as _, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, Read, Write};
const CONTINUATION_FLAG: u32 = 1_u32 << 31;
const LENGTH_MASK: u32 = !CONTINUATION_FLAG;
const CHUNK_SIZE: usize = 1024 * 1024;
pub(super) fn write<T, W>(writer: &mut W, value: &T) -> Result<()>
where
    T: Serialize,
    W: Write,
{
    let mut frame = FrameWriter::<W, CHUNK_SIZE>::new(writer);
    let buffered = sonic_rs::writer::BufferedWriter::new(&mut frame);
    sonic_rs::to_writer(buffered, value).context("failed to serialize IPC frame")?;
    frame.finish().context("failed to finish IPC frame")
}
pub(super) fn read_or_eof<T, R>(reader: &mut R) -> Result<Option<T>>
where
    T: DeserializeOwned,
    R: Read,
{
    let Some(mut frame) = FrameReader::open(reader).context("failed to read IPC frame length")?
    else {
        return Ok(None);
    };
    sonic_rs::from_reader(&mut frame)
        .map(Some)
        .context("failed to parse IPC frame")
}
struct FrameWriter<'writer, W, const SIZE: usize> {
    writer: &'writer mut W,
    pending: Vec<u8>,
}
impl<'writer, W, const SIZE: usize> FrameWriter<'writer, W, SIZE>
where
    W: Write,
{
    fn new(writer: &'writer mut W) -> Self {
        assert!(SIZE > 0, "IPC frame chunk size must be positive");
        Self {
            writer,
            pending: Vec::with_capacity(SIZE),
        }
    }
    fn finish(mut self) -> io::Result<()> {
        self.flush_chunk(false)?;
        self.writer.flush()
    }
    fn flush_chunk(&mut self, continues: bool) -> io::Result<()> {
        let length = u32::try_from(self.pending.len()).map_err(io::Error::other)?;
        if length > LENGTH_MASK {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "IPC frame chunk exceeds the encodable length",
            ));
        }
        let header = if continues {
            length | CONTINUATION_FLAG
        } else {
            length
        };
        self.writer.write_all(&header.to_be_bytes())?;
        self.writer.write_all(&self.pending)?;
        self.pending.clear();
        Ok(())
    }
}
impl<W, const SIZE: usize> Write for FrameWriter<'_, W, SIZE>
where
    W: Write,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut offset = 0_usize;
        while offset < buf.len() {
            if self.pending.len() == SIZE {
                self.flush_chunk(true)?;
            }
            let available = SIZE.saturating_sub(self.pending.len());
            let consumed = available.min(buf.len().saturating_sub(offset));
            let end = offset.saturating_add(consumed);
            let portion = buf.get(offset..end).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "invalid IPC write range")
            })?;
            self.pending.extend_from_slice(portion);
            offset = end;
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
struct FrameReader<'reader, R> {
    reader: &'reader mut R,
    remaining: usize,
    continues: bool,
    finished: bool,
}
impl<'reader, R> FrameReader<'reader, R>
where
    R: Read,
{
    fn open(reader: &'reader mut R) -> io::Result<Option<Self>> {
        let Some((remaining, continues)) = read_header(reader, true)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            reader,
            remaining,
            continues,
            finished: false,
        }))
    }
    fn advance(&mut self) -> io::Result<()> {
        if !self.continues {
            self.finished = true;
            return Ok(());
        }
        let Some((remaining, continues)) = read_header(self.reader, false)? else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "IPC stream ended before a continuation chunk",
            ));
        };
        self.remaining = remaining;
        self.continues = continues;
        Ok(())
    }
}
impl<R> Read for FrameReader<'_, R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() || self.finished {
            return Ok(0);
        }
        while self.remaining == 0 {
            self.advance()?;
            if self.finished {
                return Ok(0);
            }
        }
        let readable = self.remaining.min(buf.len());
        let target = buf
            .get_mut(..readable)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid IPC read range"))?;
        let read_count = self.reader.read(target)?;
        if read_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "IPC stream ended while reading a frame chunk",
            ));
        }
        self.remaining = self.remaining.saturating_sub(read_count);
        Ok(read_count)
    }
}
fn read_header<R>(reader: &mut R, eof_allowed: bool) -> io::Result<Option<(usize, bool)>>
where
    R: Read,
{
    let mut header = [0_u8; 4];
    let mut offset = 0_usize;
    while offset < header.len() {
        let remaining = header.get_mut(offset..).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "invalid IPC header range")
        })?;
        match reader.read(remaining) {
            Ok(0) if offset == 0 && eof_allowed => return Ok(None),
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "IPC stream ended while reading a frame length",
                ));
            }
            Ok(read_count) => offset = offset.saturating_add(read_count),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    let encoded = u32::from_be_bytes(header);
    let continues = encoded & CONTINUATION_FLAG != 0;
    let length = usize::try_from(encoded & LENGTH_MASK).map_err(io::Error::other)?;
    if continues && length == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "IPC frame contains an empty continuation chunk",
        ));
    }
    Ok(Some((length, continues)))
}
#[cfg(test)]
#[path = "../../../../tests/unit/ipc_framing.rs"]
mod tests;
