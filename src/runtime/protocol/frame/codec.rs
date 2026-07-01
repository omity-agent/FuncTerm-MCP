use anyhow::{Context as _, Result, bail};
use std::path::{Path, PathBuf};
pub(super) struct Cursor<'payload> {
    bytes: &'payload [u8],
    offset: usize,
}
impl<'payload> Cursor<'payload> {
    pub(super) const fn new(bytes: &'payload [u8]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(super) fn take_bytes(&mut self, len: u64) -> Result<&'payload [u8]> {
        let slice_len = usize::try_from(len).context("frame length does not fit usize")?;
        let end = self
            .offset
            .checked_add(slice_len)
            .context("frame length overflow")?;
        let slice = self
            .bytes
            .get(self.offset..end)
            .context("frame payload is shorter than declared")?;
        self.offset = end;
        Ok(slice)
    }
    pub(super) fn take_text(&mut self, len: u64) -> Result<String> {
        let bytes = self.take_bytes(len)?;
        String::from_utf8(bytes.to_vec()).context("frame text is not valid UTF-8")
    }
    pub(super) fn take_path(&mut self, len: u64) -> Result<PathBuf> {
        Ok(PathBuf::from(self.take_text(len)?))
    }
    pub(super) fn finish(&self) -> Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            bail!("frame payload contains trailing bytes")
        }
    }
}
pub(super) fn append_text(payload: &mut Vec<u8>, value: &str) -> Result<u64> {
    append_bytes(payload, value.as_bytes())
}
pub(super) fn append_path(payload: &mut Vec<u8>, value: &Path) -> Result<u64> {
    let text = value
        .to_str()
        .with_context(|| format!("path is not valid UTF-8: {}", value.display()))?;
    append_text(payload, text)
}
pub(super) fn append_bytes(payload: &mut Vec<u8>, value: &[u8]) -> Result<u64> {
    payload.extend_from_slice(value);
    u64::try_from(value.len()).context("payload length does not fit u64")
}
pub(super) fn decode_flag(value: u8, name: &str) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => bail!("{name} flag has invalid value {other}"),
    }
}
