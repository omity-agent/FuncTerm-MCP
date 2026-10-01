use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::powershell_wrapper;
use alloc::borrow::Cow;
use anyhow::Result;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init = initialization_script(context)?;
    let init_path = context.startup_directory.join("powershell_init.ps1");
    fs_err::write(&init_path, init)?;
    Ok(DriverStartup {
        args: vec![
            "-NoLogo".to_owned(),
            "-NoProfile".to_owned(),
            "-NoExit".to_owned(),
            "-ExecutionPolicy".to_owned(),
            "Bypass".to_owned(),
            "-File".to_owned(),
            quote::native_path(&init_path)?,
        ],
        env: Vec::new(),
    })
}
pub(super) fn command_script(command: &str) -> String {
    command.to_owned()
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    powershell_interactive_arguments(&values)
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    let initialization = format!(
        "$env:{CURRENT_SHELL_ENV} = 'powershell'\n{}\nSet-Location -LiteralPath {}\n$script:@VAR_readyWritten@ = $false\n$script:@VAR_originalPrompt@ = (Get-Command prompt).ScriptBlock\nfunction prompt {{\n    if (-not $script:@VAR_readyWritten@) {{\n        Set-Content -LiteralPath {} -Value '' -NoNewline\n        $script:@VAR_readyWritten@ = $true\n    }}\n    & $script:@VAR_originalPrompt@\n}}",
        powershell_wrapper(),
        quote::powershell_path(context.cwd)?,
        quote::powershell_path(context.ready_file)?
    );
    Ok(crate::shell::wrappers::VariableNamespace::new().render(&initialization))
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
