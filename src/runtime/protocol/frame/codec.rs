use anyhow::{Context as _, Result, bail};
use core::mem::MaybeUninit;
use std::path::Path;
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
    pub(super) fn take_str(&mut self, len: u64) -> Result<&'payload str> {
        let bytes = self.take_bytes(len)?;
        core::str::from_utf8(bytes).context("frame text is not valid UTF-8")
    }
    pub(super) fn take_text(&mut self, len: u64) -> Result<String> {
        Ok(self.take_str(len)?.to_owned())
    }
    pub(super) fn take_path_ref(&mut self, len: u64) -> Result<&'payload Path> {
        Ok(Path::new(self.take_str(len)?))
    }
    pub(super) fn finish(&self) -> Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            bail!("frame payload contains trailing bytes")
        }
    }
}
pub(super) trait PayloadSink {
    fn append_bytes(&mut self, value: &[u8]) -> Result<u64>;
    fn append_text(&mut self, value: &str) -> Result<u64> {
        self.append_bytes(value.as_bytes())
    }
    fn append_path(&mut self, value: &Path) -> Result<u64> {
        let text = value
            .to_str()
            .with_context(|| format!("path is not valid UTF-8: {}", value.display()))?;
        self.append_text(text)
    }
}
#[derive(Default)]
pub(super) struct PayloadSize {
    len: usize,
}
impl PayloadSize {
    pub(super) const fn len(&self) -> usize {
        self.len
    }
}
impl PayloadSink for PayloadSize {
    fn append_bytes(&mut self, value: &[u8]) -> Result<u64> {
        self.len = self
            .len
            .checked_add(value.len())
            .context("payload length overflow")?;
        u64::try_from(value.len()).context("payload length does not fit u64")
    }
    fn append_text(&mut self, value: &str) -> Result<u64> {
        self.append_bytes(value.as_bytes())
    }
    fn append_path(&mut self, value: &Path) -> Result<u64> {
        let text = value
            .to_str()
            .with_context(|| format!("path is not valid UTF-8: {}", value.display()))?;
        self.append_text(text)
    }
}
#[cfg(test)]
pub(super) struct PayloadVec {
    bytes: Vec<u8>,
}
#[cfg(test)]
impl PayloadVec {
    pub(super) const fn new() -> Self {
        Self { bytes: Vec::new() }
    }
    pub(super) fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}
#[cfg(test)]
impl PayloadSink for PayloadVec {
    fn append_bytes(&mut self, value: &[u8]) -> Result<u64> {
        self.bytes.extend_from_slice(value);
        u64::try_from(value.len()).context("payload length does not fit u64")
    }
    fn append_text(&mut self, value: &str) -> Result<u64> {
        self.append_bytes(value.as_bytes())
    }
    fn append_path(&mut self, value: &Path) -> Result<u64> {
        let text = value
            .to_str()
            .with_context(|| format!("path is not valid UTF-8: {}", value.display()))?;
        self.append_text(text)
    }
}
pub(super) struct PayloadWriter<'payload> {
    bytes: &'payload mut [MaybeUninit<u8>],
    offset: usize,
}
impl<'payload> PayloadWriter<'payload> {
    pub(super) const fn new(bytes: &'payload mut [MaybeUninit<u8>]) -> Self {
        Self { bytes, offset: 0 }
    }
    pub(super) fn finish(&self) -> Result<()> {
        if self.offset == self.bytes.len() {
            Ok(())
        } else {
            bail!("frame payload writer left uninitialized bytes")
        }
    }
}
impl PayloadSink for PayloadWriter<'_> {
    fn append_bytes(&mut self, value: &[u8]) -> Result<u64> {
        let end = self
            .offset
            .checked_add(value.len())
            .context("payload length overflow")?;
        let target = self
            .bytes
            .get_mut(self.offset..end)
            .context("payload writer buffer is shorter than encoded frame")?;
        unsafe {
            core::ptr::copy_nonoverlapping(value.as_ptr(), target.as_mut_ptr().cast(), value.len());
        }
        self.offset = end;
        u64::try_from(value.len()).context("payload length does not fit u64")
    }
    fn append_text(&mut self, value: &str) -> Result<u64> {
        self.append_bytes(value.as_bytes())
    }
    fn append_path(&mut self, value: &Path) -> Result<u64> {
        let text = value
            .to_str()
            .with_context(|| format!("path is not valid UTF-8: {}", value.display()))?;
        self.append_text(text)
    }
}
pub(super) fn decode_flag(value: u8, name: &str) -> Result<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        other => bail!("{name} flag has invalid value {other}"),
    }
}
