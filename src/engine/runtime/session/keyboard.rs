use alloc::borrow::Cow;
#[cfg(windows)]
use anyhow::Context as _;
use anyhow::Result;
const ETX: u8 = 0x03;
#[cfg(not(windows))]
const ETX_BYTES: &[u8] = &[ETX];
pub(super) struct InputBatch {
    events: Vec<InputEvent>,
    interrupted: bool,
}
pub(super) enum InputEvent {
    Data(Vec<u8>),
    Interrupt,
}
#[derive(Clone, Copy)]
pub(super) struct InputDelivery {
    interrupted: bool,
}
impl InputBatch {
    pub(super) fn from_bytes(bytes: &[u8]) -> Self {
        let mut events = Vec::new();
        let mut interrupted = false;
        for chunk in bytes.split_inclusive(|byte| *byte == ETX) {
            let data = chunk.strip_suffix(&[ETX]).unwrap_or(chunk);
            let chunk_interrupted = data.len() != chunk.len();
            if !data.is_empty() {
                events.push(InputEvent::Data(data.to_vec()));
            }
            if chunk_interrupted {
                events.push(InputEvent::Interrupt);
                interrupted = true;
            }
        }
        Self {
            events,
            interrupted,
        }
    }
    pub(super) fn events(&self) -> &[InputEvent] {
        &self.events
    }
    pub(super) const fn delivery(&self) -> InputDelivery {
        InputDelivery {
            interrupted: self.interrupted,
        }
    }
}
impl InputDelivery {
    pub(super) const fn interrupted(self) -> bool {
        self.interrupted
    }
}
pub(super) fn user_bytes(event: &InputEvent) -> &[u8] {
    platform_user_bytes(event)
}
pub(super) fn host_reply_bytes(bytes: &[u8], win32_input: bool) -> Result<Cow<'_, [u8]>> {
    platform_host_reply_bytes(bytes, win32_input)
}
#[cfg(not(windows))]
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching a borrowed input event keeps its data allocation borrowed"
)]
fn platform_user_bytes(event: &InputEvent) -> &[u8] {
    match event {
        InputEvent::Data(bytes) => bytes,
        InputEvent::Interrupt => ETX_BYTES,
    }
}
#[cfg(not(windows))]
fn platform_host_reply_bytes(bytes: &[u8], _win32_input: bool) -> Result<Cow<'_, [u8]>> {
    Ok(Cow::Borrowed(bytes))
}
#[cfg(windows)]
#[expect(
    clippy::pattern_type_mismatch,
    reason = "matching a borrowed input event keeps its data allocation borrowed"
)]
fn platform_user_bytes(event: &InputEvent) -> &[u8] {
    match event {
        InputEvent::Data(bytes) => bytes,
        InputEvent::Interrupt => ctrl_c_sequence(),
    }
}
#[cfg(windows)]
fn platform_host_reply_bytes(bytes: &[u8], win32_input: bool) -> Result<Cow<'_, [u8]>> {
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
const fn ctrl_c_sequence() -> &'static [u8] {
    concat!(
        "\x1b[17;29;0;1;8;1_",
        "\x1b[67;46;3;1;8;1_",
        "\x1b[67;46;3;0;8;1_",
        "\x1b[17;29;0;0;0;1_",
    )
    .as_bytes()
}
#[cfg(test)]
mod tests {
    use super::{InputBatch, user_bytes};
    fn encoded_input(bytes: &[u8]) -> (Vec<u8>, super::InputDelivery) {
        let batch = InputBatch::from_bytes(bytes);
        let mut encoded = Vec::new();
        for event in batch.events() {
            encoded.extend_from_slice(user_bytes(event));
        }
        (encoded, batch.delivery())
    }
    #[test]
    fn ordinary_text_is_unchanged() {
        let (encoded, delivery) = encoded_input(b"typed input");
        assert_eq!(encoded, b"typed input");
        assert!(!delivery.interrupted());
    }
    #[cfg(not(windows))]
    #[test]
    fn terminal_reply_is_unchanged_on_unix() {
        assert_eq!(
            super::host_reply_bytes(b"\x1b[1;2R", true)
                .unwrap()
                .as_ref(),
            b"\x1b[1;2R"
        );
    }
    #[cfg(windows)]
    #[test]
    fn ctrl_c_uses_win32_input_mode_key_events() {
        let (encoded, delivery) = encoded_input(b"\x03");
        assert_eq!(
            encoded,
            concat!(
                "\x1b[17;29;0;1;8;1_",
                "\x1b[67;46;3;1;8;1_",
                "\x1b[67;46;3;0;8;1_",
                "\x1b[17;29;0;0;0;1_",
            )
            .as_bytes()
        );
        assert!(delivery.interrupted());
    }
    #[cfg(windows)]
    #[test]
    fn ctrl_c_can_be_embedded_between_text_chunks() {
        let (encoded, delivery) = encoded_input(b"before\x03after");
        assert_eq!(
            encoded,
            concat!(
                "before",
                "\x1b[17;29;0;1;8;1_",
                "\x1b[67;46;3;1;8;1_",
                "\x1b[67;46;3;0;8;1_",
                "\x1b[17;29;0;0;0;1_",
                "after",
            )
            .as_bytes()
        );
        assert!(delivery.interrupted());
    }
    #[cfg(not(windows))]
    #[test]
    fn ctrl_c_remains_etx_on_unix() {
        let (encoded, delivery) = encoded_input(b"before\x03after");
        assert_eq!(encoded, b"before\x03after");
        assert!(delivery.interrupted());
    }
    #[cfg(windows)]
    #[test]
    fn terminal_reply_is_encoded_as_literal_win32_input() {
        assert_eq!(
            super::host_reply_bytes(b"\x1b[1;2R", true)
                .unwrap()
                .as_ref(),
            concat!(
                "\x1b[0;0;27;1;0;1_",
                "\x1b[0;0;91;1;0;1_",
                "\x1b[0;0;49;1;0;1_",
                "\x1b[0;0;59;1;0;1_",
                "\x1b[0;0;50;1;0;1_",
                "\x1b[0;0;82;1;0;1_",
            )
            .as_bytes()
        );
    }
}
