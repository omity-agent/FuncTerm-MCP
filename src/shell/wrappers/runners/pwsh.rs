mod console_bridge;
mod helper_process;
mod stream_capture;
mod submission_state;
use super::super::{VariableNamespace, template, variables};
pub(in crate::shell) fn wrapper() -> String {
    let managed = [
        helper_process::SOURCE,
        stream_capture::SOURCE,
        console_bridge::SOURCE,
        submission_state::SOURCE,
    ]
    .join("\n");
    let script = format!("{IMPORTS}\n{managed}\n'@\n{INPUT}");
    let protected = script.replace(
        "@POWERSHELL_PROTECTED_ENVIRONMENT@",
        &variables::powershell_protected_environment_names(),
    );
    let dispatched = protected.replace(
        "@DISPATCHER@",
        crate::contract::POWERSHELL_DISPATCHER_FUNCTION,
    );
    VariableNamespace::new().render(&template::render_powershell(&dispatched))
}
const IMPORTS: &str = "Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Linq;
using System.Diagnostics;
using System.Runtime.InteropServices;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Globalization;
using System.Management.Automation;
using System.Management.Automation.Host;
using System.Management.Automation.Runspaces;
using System.Reflection;
using System.Security;";
const INPUT: &str = r"Import-Module -Name PSReadLine -ErrorAction Stop
Set-PSReadLineOption -HistorySaveStyle SaveNothing -AddToHistoryHandler {
    param($@VAR_line@)
    $@VAR_line@ -ne '. @DISPATCHER@'
}
Set-PSReadLineKeyHandler -Chord Enter -Function AcceptLine
$script:@VAR_submission@ = [FuncTermSubmission]::new(
    $Host.Runspace, $Host, $ExecutionContext, @(@POWERSHELL_PROTECTED_ENVIRONMENT@)
)
function global:PSConsoleHostReadLine {
    $@VAR_previousStatus@ = $?
    Microsoft.PowerShell.Core\Set-StrictMode -Off
    $@VAR_line@ = [Microsoft.PowerShell.PSConsoleReadLine]::ReadLine(
        $Host.Runspace, $ExecutionContext, $@VAR_previousStatus@
    )
    if ($@VAR_line@ -ne '. @DISPATCHER@') { return $@VAR_line@ }
    $@VAR_receipt@ = $script:@VAR_submission@.Claim() | Microsoft.PowerShell.Utility\ConvertFrom-Json
    $@VAR_source@ = $script:@VAR_submission@.Accept(
        $@VAR_receipt@.command_id, $@VAR_receipt@.directory, $@VAR_receipt@.separate_output
    )
    [Microsoft.PowerShell.PSConsoleReadLine]::AddToHistory($@VAR_source@)
    $@VAR_source@
}";
