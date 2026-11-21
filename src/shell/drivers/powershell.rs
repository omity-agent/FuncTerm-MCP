use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{powershell_wrapper, startup_environment};
use alloc::borrow::Cow;
use anyhow::Result;
const REQUIRE_CORE: &str = "if ($PSVersionTable.PSVersion.Major -lt 7) {
    [Console]::Error.WriteLine('FuncTerm requires PowerShell 7 or newer (pwsh).')
    exit 1
}";
const SET_CULTURE: &str = "[Globalization.CultureInfo]::DefaultThreadCurrentCulture = [Globalization.CultureInfo]::GetCultureInfo('en-US')
[Globalization.CultureInfo]::DefaultThreadCurrentUICulture = [Globalization.CultureInfo]::DefaultThreadCurrentCulture
[Globalization.CultureInfo]::CurrentCulture = [Globalization.CultureInfo]::DefaultThreadCurrentCulture
[Globalization.CultureInfo]::CurrentUICulture = [Globalization.CultureInfo]::DefaultThreadCurrentUICulture";
const LOAD_PROFILES: &str = r"foreach ($@VAR_profilePath@ in @(
    $PROFILE.AllUsersAllHosts, $PROFILE.AllUsersCurrentHost,
    $PROFILE.CurrentUserAllHosts, $PROFILE.CurrentUserCurrentHost
)) {
    if ([IO.File]::Exists($@VAR_profilePath@)) {
        try {
            . $@VAR_profilePath@
        } catch {
            Microsoft.PowerShell.Utility\Write-Error -ErrorRecord $_ -ErrorAction Continue
        }
    }
}";
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init = initialization_script(context)?;
    let init_path = context.startup_directory.join("powershell_init.ps1");
    fs_err::write(&init_path, init)?;
    let args = vec![
        "-NoLogo".to_owned(),
        "-NoProfile".to_owned(),
        "-NoExit".to_owned(),
        "-ExecutionPolicy".to_owned(),
        "Bypass".to_owned(),
        "-File".to_owned(),
        quote::native_path(&init_path)?,
    ];
    Ok(DriverStartup {
        args,
        env: Vec::new(),
    })
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    powershell_interactive_arguments(&values)
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    let profiles = if context.load_profile {
        LOAD_PROFILES
    } else {
        ""
    };
    let style = if context.ignore_shell_style {
        r"$env:NO_COLOR = '1'
if (Microsoft.PowerShell.Utility\Get-Variable PSStyle -ErrorAction SilentlyContinue) {
    $PSStyle.OutputRendering = 'PlainText'
}"
    } else {
        ""
    };
    let prompt = if context.ignore_shell_style {
        format!(
            "function prompt {{
    if (-not $script:@VAR_readyWritten@) {{
        [IO.File]::WriteAllText({}, '')
        $script:@VAR_readyWritten@ = $true
    }}
    'PS> '
}}",
            quote::powershell_path(context.ready_file)?
        )
    } else {
        format!(
            r"$script:@VAR_originalPrompt@ = (Microsoft.PowerShell.Core\Get-Command prompt -CommandType Function).ScriptBlock
function prompt {{
    $@VAR_promptText@ = & $script:@VAR_originalPrompt@
    if (-not $script:@VAR_readyWritten@) {{
        [IO.File]::WriteAllText({}, '')
        $script:@VAR_readyWritten@ = $true
    }}
    $@VAR_promptText@
}}",
            quote::powershell_path(context.ready_file)?
        )
    };
    let initialization = format!(
        "{REQUIRE_CORE}\n{SET_CULTURE}\n{profiles}\n{}\n$env:{CURRENT_SHELL_ENV} = 'powershell'\n{}\n{}\nMicrosoft.PowerShell.Management\\Set-Location -LiteralPath {}\n$script:@VAR_readyWritten@ = $false\n{}",
        startup_environment(crate::shell::ShellChoice::PowerShell, context.environment)?,
        powershell_wrapper(),
        style,
        quote::powershell_path(context.cwd)?,
        prompt
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
