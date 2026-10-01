use alloc::borrow::Cow;
#[cfg(windows)]
use anyhow::Context as _;
use anyhow::Result;
const ETX: u8 = 0x03;
#[cfg(not(windows))]
const ETX_BYTES: &[u8] = &[ETX];
pub(super) struct InputBatch<'input> {
    bytes: &'input [u8],
}
#[derive(Clone, Copy)]
pub(super) struct InputDelivery {
    interrupted: bool,
}
impl<'input> InputBatch<'input> {
    pub(super) const fn from_bytes(bytes: &'input [u8]) -> Self {
        Self { bytes }
    }
    pub(super) fn segments(&self) -> impl Iterator<Item = &'input [u8]> {
        self.bytes
            .split_inclusive(|byte| *byte == ETX)
            .flat_map(|chunk| {
                chunk
                    .strip_suffix(&[ETX])
                    .map_or([chunk, &[]], |data| [data, interrupt_bytes()])
            })
            .filter(|segment| !segment.is_empty())
    }
    pub(super) fn delivery(&self) -> InputDelivery {
        InputDelivery {
            interrupted: self.bytes.contains(&ETX),
        }
    }
}
impl InputDelivery {
    pub(super) const fn interrupted(self) -> bool {
        self.interrupted
    }
}
#[cfg(not(windows))]
const fn interrupt_bytes() -> &'static [u8] {
    ETX_BYTES
}
#[cfg(not(windows))]
#[expect(
    clippy::unnecessary_wraps,
    reason = "the cross-platform reply interface can fail during Windows input encoding"
)]
pub(super) const fn host_reply_bytes(bytes: &[u8], _win32_input: bool) -> Result<Cow<'_, [u8]>> {
    Ok(Cow::Borrowed(bytes))
}
#[cfg(windows)]
pub(super) fn host_reply_bytes(bytes: &[u8], win32_input: bool) -> Result<Cow<'_, [u8]>> {
    use core::fmt::Write as _;
    if !win32_input {
        return Ok(Cow::Borrowed(bytes));
    }
    let text = core::str::from_utf8(bytes).context("terminal host reply is not valid UTF-8")?;
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(20));
    for code_unit in text.encode_utf16() {
        write!(encoded, "\x1b[0;0;{code_unit};1;0;1_")
            .context("failed to encode terminal host reply as Win32 input")?;
    }
    Ok(Cow::Owned(encoded.into_bytes()))
}
#[cfg(windows)]
const fn interrupt_bytes() -> &'static [u8] {
    concat!(
        "\x1b[17;29;0;1;8;1_",
        "\x1b[67;46;3;1;8;1_",
        "\x1b[67;46;3;0;8;1_",
        "\x1b[17;29;0;0;0;1_",
    )
    .as_bytes()
}
