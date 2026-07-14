use alloc::borrow::Cow;
#[cfg(windows)]
use anyhow::Context as _;
use anyhow::Result;
#[cfg(windows)]
const ETX: u8 = 0x03;
pub(super) fn user_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    platform_user_bytes(bytes)
}
pub(super) fn host_reply_bytes(bytes: &[u8], win32_input: bool) -> Result<Cow<'_, [u8]>> {
    platform_host_reply_bytes(bytes, win32_input)
}
#[cfg(not(windows))]
fn platform_user_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    Cow::Borrowed(bytes)
}
#[cfg(not(windows))]
fn platform_host_reply_bytes(bytes: &[u8], _win32_input: bool) -> Result<Cow<'_, [u8]>> {
    Ok(Cow::Borrowed(bytes))
}
#[cfg(windows)]
fn platform_user_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&ETX) {
        return Cow::Borrowed(bytes);
    }
    let mut encoded = Vec::with_capacity(bytes.len() + ctrl_c_sequence().len());
    for byte in bytes {
        if *byte == ETX {
            encoded.extend_from_slice(ctrl_c_sequence());
        } else {
            encoded.push(*byte);
        }
    }
    Cow::Owned(encoded)
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
    use super::user_bytes;
    #[test]
    fn ordinary_text_is_unchanged() {
        assert_eq!(user_bytes(b"typed input").as_ref(), b"typed input");
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
        assert_eq!(
            user_bytes(b"\x03").as_ref(),
            concat!(
                "\x1b[17;29;0;1;8;1_",
                "\x1b[67;46;3;1;8;1_",
                "\x1b[67;46;3;0;8;1_",
                "\x1b[17;29;0;0;0;1_",
            )
            .as_bytes()
        );
    }
    #[cfg(windows)]
    #[test]
    fn ctrl_c_can_be_embedded_between_text_chunks() {
        assert_eq!(
            user_bytes(b"before\x03after").as_ref(),
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
