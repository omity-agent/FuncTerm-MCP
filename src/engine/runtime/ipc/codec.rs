use anyhow::{Context as _, Result};
use serde::{Serialize, de::DeserializeOwned};
use std::io::{self, BufRead, BufWriter, Write};
pub(super) fn write<T, W>(writer: &mut W, value: &T) -> Result<()>
where
    T: Serialize,
    W: Write,
{
    let mut buffered = BufWriter::new(writer);
    value
        .serialize(&mut rmp_serde::Serializer::new(&mut buffered))
        .context("failed to serialize IPC message")?;
    buffered.flush().context("failed to flush IPC message")
}
pub(super) fn read_or_eof<T, R>(reader: &mut R) -> Result<Option<T>>
where
    T: DeserializeOwned,
    R: BufRead,
{
    loop {
        match reader.fill_buf() {
            Ok(&[]) => return Ok(None),
            Ok(_bytes) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error).context("failed to read IPC message"),
        }
    }
    rmp_serde::from_read(reader)
        .map(Some)
        .context("failed to decode IPC message")
}
