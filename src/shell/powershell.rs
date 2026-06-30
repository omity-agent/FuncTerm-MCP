use base64_turbo::STANDARD;
use std::path::Path;
use uuid::Uuid;
pub(super) fn startup_args(cwd: &Path) -> Vec<String> {
    let init = initialization_script(cwd);
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
pub(super) fn invocation(command_id: Uuid, command: &str, directory: &Path) -> String {
    let payload = STANDARD.encode(command.as_bytes());
    let quoted_directory = ps_quote(directory);
    format!(
        "Invoke-McpPtyCommand -CommandId '{command_id}' -Payload '{payload}' -Directory {quoted_directory}\r\n"
    )
}
fn initialization_script(cwd: &Path) -> String {
    format!(
        "{}\nSet-Location -LiteralPath {}",
        include_str!("../powershell_init.ps1"),
        ps_quote(cwd)
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
        let script = super::initialization_script(Path::new("F:\\dir with ' quote"));
        assert!(script.contains("Invoke-McpPtyCommand"));
        assert!(script.contains("Set-Location -LiteralPath 'F:\\dir with '' quote'"));
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
}
