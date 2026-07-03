use super::{DriverStartup, InvocationContext, ShellDriver, StartupContext, os_strings_lower};
use crate::contract::POWERSHELL_COMMAND_FUNCTION;
use crate::runtime::config::Settings;
use crate::shell::ShellChoice;
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::powershell_wrapper;
use alloc::borrow::Cow;
use anyhow::Result;
use base64_turbo::STANDARD;
pub(crate) struct PowerShellDriver;
impl ShellDriver for PowerShellDriver {
    fn choice(&self) -> ShellChoice {
        ShellChoice::PowerShell
    }
    fn id(&self) -> &'static str {
        "powershell"
    }
    fn aliases(&self) -> &'static [&'static str] {
        &[
            "pwsh",
            "pwsh.exe",
            "powershell",
            "powershell.exe",
            "powershell_core",
            "windows_powershell",
        ]
    }
    fn shim_env_name(&self) -> &'static str {
        "FUNCTERM_REAL_POWERSHELL"
    }
    fn executable_candidates(&self, settings: &Settings) -> Result<Vec<String>> {
        Ok(settings.powershell.clone())
    }
    fn startup(&self, context: StartupContext<'_>) -> Result<DriverStartup> {
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
    fn invocation(&self, context: InvocationContext<'_>) -> Result<String> {
        let quoted_directory = quote::powershell_path(context.directory)?;
        let quoted_cwd = quote::powershell_path(context.cwd)?;
        let quoted_command_id = quote::powershell_string(context.command_id);
        Ok(format!(
            "{POWERSHELL_COMMAND_FUNCTION} -CommandId {quoted_command_id} -Directory {quoted_directory} -WorkingDirectory {quoted_cwd}\r\n"
        ))
    }
    fn keyboard_bytes<'bytes>(&self, bytes: &'bytes [u8]) -> Cow<'bytes, [u8]> {
        keyboard_bytes(bytes)
    }
    fn interactive_arguments(&self, arguments: &[std::ffi::OsString]) -> bool {
        let Some(values) = os_strings_lower(arguments) else {
            return false;
        };
        powershell_interactive_arguments(&values)
    }
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    Ok(format!(
        "$env:{CURRENT_SHELL_ENV} = 'powershell'\n{}\nSet-Location -LiteralPath {}\n$script:FuncTermReadyWritten = $false\n$script:FuncTermOriginalPrompt = (Get-Command prompt).ScriptBlock\nfunction prompt {{\n    if (-not $script:FuncTermReadyWritten) {{\n        Set-Content -LiteralPath {} -Value '' -NoNewline\n        $script:FuncTermReadyWritten = $true\n    }}\n    & $script:FuncTermOriginalPrompt\n}}",
        powershell_wrapper(),
        quote::powershell_path(context.cwd)?,
        quote::powershell_path(context.ready_file)?
    ))
}
fn keyboard_bytes(bytes: &[u8]) -> Cow<'_, [u8]> {
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
    use super::PowerShellDriver;
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
        assert!(script.contains("Invoke-FuncTermCommand"));
        assert!(script.contains("Set-Location -LiteralPath ([Text.Encoding]::UTF8.GetString"));
        assert!(script.contains("Set-Content -LiteralPath ([Text.Encoding]::UTF8.GetString"));
    }
    #[test]
    fn invocation_references_payload_file_by_directory() {
        let line = crate::shell::drivers::ShellDriver::invocation(
            &PowerShellDriver,
            crate::shell::drivers::InvocationContext {
                command_id: "command",
                directory: Path::new("F:\\dir with ' quote"),
                cwd: Path::new("F:\\cwd"),
            },
        )
        .unwrap();
        assert!(!line.contains("-Payload"));
        assert!(line.contains("-Directory ([Text.Encoding]::UTF8.GetString"));
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
