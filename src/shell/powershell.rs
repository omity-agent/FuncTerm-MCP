use alloc::borrow::Cow;
use base64_turbo::STANDARD;
use std::path::Path;
pub(super) fn startup_args(cwd: &Path, ready_file: &Path) -> Vec<String> {
    let init = initialization_script(cwd, ready_file);
    vec![
        "-NoLogo".to_owned(),
        "-NoProfile".to_owned(),
        "-NoExit".to_owned(),
        "-ExecutionPolicy".to_owned(),
        "Bypass".to_owned(),
        "-EncodedCommand".to_owned(),
        encode_command(&init),
    ]
}
pub(super) fn invocation(command_id: &str, command: &str, directory: &Path, cwd: &Path) -> String {
    let payload = STANDARD.encode(command.as_bytes());
    let quoted_directory = ps_quote(directory);
    let quoted_cwd = ps_quote(cwd);
    format!(
        "Invoke-McpPtyCommand -CommandId '{command_id}' -Payload '{payload}' -Directory {quoted_directory} -WorkingDirectory {quoted_cwd}\r\n"
    )
}
pub(super) fn keyboard_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&b'\n') {
        return Cow::Borrowed(bytes);
    }
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut previous = None;
    for byte in bytes {
        if *byte == b'\n' && previous != Some(b'\r') {
            normalized.push(b'\r');
        }
        normalized.push(*byte);
        previous = Some(*byte);
    }
    Cow::Owned(normalized)
}
fn initialization_script(cwd: &Path, ready_file: &Path) -> String {
    format!(
        "{}\nSet-Location -LiteralPath {}\nSet-Content -LiteralPath {} -Value '' -NoNewline",
        include_str!("./powershell_init.ps1"),
        ps_quote(cwd),
        ps_quote(ready_file)
    )
}
fn ps_quote(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\'', "''");
    format!("'{text}'")
}
fn encode_command(command: &str) -> String {
    let bytes = command
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    STANDARD.encode(&bytes)
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use std::path::Path;
    #[test]
    fn quotes_literal_paths_for_powershell() {
        let quoted = super::ps_quote(Path::new("F:\\dir with ' quote"));
        assert_eq!(quoted, "'F:\\dir with '' quote'");
    }
    #[test]
    fn initialization_sets_literal_location() {
        let script = super::initialization_script(
            Path::new("F:\\dir with ' quote"),
            Path::new("F:\\ready'file"),
        );
        assert!(script.contains("Invoke-McpPtyCommand"));
        assert!(script.contains("Set-Location -LiteralPath 'F:\\dir with '' quote'"));
        assert!(script.contains("Set-Content -LiteralPath 'F:\\ready''file'"));
    }
    #[test]
    fn encoded_command_round_trips_as_utf16() {
        let encoded = super::encode_command("Write-Output '中文'");
        let bytes = base64_turbo::STANDARD.decode(encoded).unwrap();
        let chunks = bytes.chunks_exact(2);
        assert!(chunks.remainder().is_empty());
        let words = chunks
            .map(|chunk| {
                let [low, high]: [u8; 2] = chunk.try_into().unwrap();
                u16::from(low) | (u16::from(high) << 8_u32)
            })
            .collect::<Vec<_>>();
        let decoded = String::from_utf16(&words).unwrap();
        assert_eq!(decoded, "Write-Output '中文'");
    }
    #[test]
    fn keyboard_input_submits_lone_line_feeds_as_enter() {
        assert_eq!(
            super::keyboard_bytes(b"exit\n").as_ref(),
            b"exit\r\n".as_slice()
        );
        assert_eq!(
            super::keyboard_bytes(b"exit\r\n").as_ref(),
            b"exit\r\n".as_slice()
        );
    }
}
