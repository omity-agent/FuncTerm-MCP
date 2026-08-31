use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::powershell_wrapper;
use alloc::borrow::Cow;
use anyhow::Result;
use base64_turbo::STANDARD;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init = initialization_script(context)?;
    Ok(DriverStartup {
        args: vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-NoExit".to_owned(),
            "-ExecutionPolicy".to_owned(),
            "Bypass".to_owned(),
            "-EncodedCommand".to_owned(),
            encode_command(&init),
        ],
        env: Vec::new(),
    })
}
pub(super) fn command_script(command: &str) -> String {
    format!(
        "$script:FuncTermCommandScript = [scriptblock]::Create({})",
        quote::powershell_string(command)
    )
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    powershell_interactive_arguments(&values)
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    Ok(format!(
        "$env:{CURRENT_SHELL_ENV} = 'powershell'\n{}\nSet-Location -LiteralPath {}\n$script:FuncTermReadyWritten = $false\n$script:FuncTermOriginalPrompt = (Get-Command prompt).ScriptBlock\nfunction prompt {{\n    if (-not $script:FuncTermReadyWritten) {{\n        Set-Content -LiteralPath {} -Value '' -NoNewline\n        $script:FuncTermReadyWritten = $true\n    }}\n    & $script:FuncTermOriginalPrompt\n}}",
        powershell_wrapper(),
        quote::powershell_path(context.cwd)?,
        quote::powershell_path(context.ready_file)?
    ))
}
pub(super) fn keyboard_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
    if !bytes.contains(&b'\n') {
        return Cow::Borrowed(bytes);
    }
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut previous = None;
    for byte in bytes {
        if *byte == b'\n' {
            if previous != Some(b'\r') {
                normalized.push(b'\r');
            }
        } else {
            normalized.push(*byte);
        }
        previous = Some(*byte);
    }
    Cow::Owned(normalized)
}
#[expect(
    clippy::little_endian_bytes,
    reason = "PowerShell EncodedCommand requires UTF-16LE on every host architecture"
)]
fn encode_command(command: &str) -> String {
    let bytes = command
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect::<Vec<_>>();
    STANDARD.encode(&bytes)
}
fn powershell_interactive_arguments(values: &[String]) -> bool {
    let mut index = 0_usize;
    while index < values.len() {
        let Some(value) = values.get(index) else {
            return false;
        };
        match value.as_str() {
            "-nologo" | "-noexit" | "-noprofile" => index += 1,
            "-executionpolicy" if index + 1 < values.len() => index += 2,
            _ => return false,
        }
    }
    true
}
#[cfg(test)]
mod tests {
    use crate::shell::ShellChoice;
    use crate::shell::drivers::StartupContext;
    use std::path::Path;
    #[test]
    fn initialization_sets_literal_location() {
        let script = super::initialization_script(StartupContext {
            cwd: Path::new("F:\\dir with ' quote"),
            startup_directory: Path::new("F:\\session\\startup"),
            ready_file: Path::new("F:\\ready'file"),
        })
        .unwrap();
        assert!(script.contains("function f"));
        assert!(script.contains("Set-Location -LiteralPath ([Text.Encoding]::UTF8.GetString"));
        assert!(script.contains("Set-Content -LiteralPath ([Text.Encoding]::UTF8.GetString"));
    }
    #[test]
    fn invocation_is_short_dispatcher() {
        let invocation = crate::shell::drivers::invocation(ShellChoice::PowerShell)
            .unwrap()
            .unwrap();
        let bytes = invocation.into_bytes();
        assert_eq!(bytes, b"f\r");
    }
    #[test]
    fn keyboard_input_encodes_each_line_break_as_one_enter() {
        assert_eq!(
            super::keyboard_bytes(b"exit\n").as_ref(),
            b"exit\r".as_slice()
        );
        assert_eq!(
            super::keyboard_bytes(b"exit\r\n").as_ref(),
            b"exit\r".as_slice()
        );
    }
}
