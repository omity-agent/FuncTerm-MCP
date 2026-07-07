use alloc::borrow::Cow;
#[cfg(windows)]
const ETX: u8 = 0x03;
pub(super) fn physical_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    platform_physical_bytes(bytes)
}
pub(super) fn requests_interrupt(bytes: &[u8]) -> bool {
    platform_requests_interrupt(bytes)
}
#[cfg(not(windows))]
fn platform_physical_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    Cow::Borrowed(bytes)
}
#[cfg(not(windows))]
const fn platform_requests_interrupt(_bytes: &[u8]) -> bool {
    false
}
#[cfg(windows)]
fn platform_physical_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
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
fn platform_requests_interrupt(bytes: &[u8]) -> bool {
    bytes.contains(&ETX)
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
    use super::physical_bytes;
    #[test]
    fn ordinary_text_is_unchanged() {
        assert_eq!(physical_bytes(b"typed input").as_ref(), b"typed input");
    }
    #[test]
    fn ordinary_text_does_not_request_interrupt() {
        assert!(!super::requests_interrupt(b"typed input"));
    }
    #[cfg(windows)]
    #[test]
    fn ctrl_c_uses_win32_input_mode_key_events() {
        assert_eq!(
            physical_bytes(b"\x03").as_ref(),
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
    fn ctrl_c_requests_interrupt() {
        assert!(super::requests_interrupt(b"\x03"));
    }
    #[cfg(windows)]
    #[test]
    fn ctrl_c_can_be_embedded_between_text_chunks() {
        assert_eq!(
            physical_bytes(b"before\x03after").as_ref(),
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
}
