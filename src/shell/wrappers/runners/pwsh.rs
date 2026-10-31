use super::super::{VariableNamespace, start, template, variables};
use crate::contract::POWERSHELL_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    let script = format!(
        "{}\n{}\n{}\n{}\n{COMMAND_TEMPLATE}\n{}",
        start::POWERSHELL,
        start::POWERSHELL_SHIMS,
        super::ordered_streams::SCRIPT,
        super::instrumentation::SCRIPT,
        template::powershell_dispatcher()
    );
    let rendered = template::render_command_function(&script, POWERSHELL_COMMAND_FUNCTION);
    let protected = rendered.replace(
        "@POWERSHELL_PROTECTED_ENVIRONMENT@",
        &variables::powershell_protected_environment_names(),
    );
    VariableNamespace::new().render_powershell(&template::render_powershell(&protected))
}
const COMMAND_TEMPLATE: &str = r"& {
    if (Get-Command Set-PSReadLineOption -ErrorAction SilentlyContinue) {
        Set-PSReadLineOption -HistorySaveStyle SaveNothing
        if ((Get-Command Set-PSReadLineOption).Parameters.ContainsKey('AddToHistoryHandler')) {
            Set-PSReadLineOption -AddToHistoryHandler { $false }
        }
    }
    Clear-History
}
function @FUNCTION@ {
    param(
        [Parameter(Mandatory = $true)][string]$@VAR_CommandId@,
        [Parameter(Mandatory = $true)][string]$@VAR_Directory@,
        [Parameter(Mandatory = $true)][string]$@VAR_WorkingDirectory@,
        [Parameter(Mandatory = $true)][bool]$@VAR_SeparateOutput@
    )
    $@VAR_stderrFile@ = Join-Path $@VAR_Directory@ '@STDERR@'
    $@VAR_scriptFile@ = Join-Path $@VAR_Directory@ '@SCRIPT@'
    $@VAR_previousCommandId@ = $env:@COMMAND_ID_ENV@
    $@VAR_previousCommandDirectory@ = $env:@COMMAND_DIR_ENV@
    $@VAR_protectedEnvironment@ = [Collections.Generic.Dictionary[string, string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($@VAR_entry@ in Get-ChildItem Env:) {
        $@VAR_protectedEnvironment@[$@VAR_entry@.Name] = $@VAR_entry@.Value
    }
    $@VAR_exitCode@ = 1
    $@VAR_failure@ = $null
    $@VAR_pipeline@ = $null
    $@VAR_capture@ = $null
    $@VAR_invocationSettings@ = $null
    $@VAR_nativeVariable@ = $null
    $@VAR_nativeWrite@ = [FuncTermNativeWrite]::new()
    $@VAR_timer@ = [Diagnostics.Stopwatch]::new()
    $env:@COMMAND_ID_ENV@ = $@VAR_CommandId@
    $env:@COMMAND_DIR_ENV@ = $@VAR_Directory@
    try {
        Set-FuncTermShimPath
        Set-Location -LiteralPath $@VAR_WorkingDirectory@
        Publish-FuncTermStart -@VAR_CommandId@ $@VAR_CommandId@ -@VAR_Directory@ $@VAR_Directory@
        $@VAR_succeeded@ = $false
        $@VAR_nativeCode@ = $null
        $@VAR_returned@ = $false
        $@VAR_timer@.Start()
        $@VAR_nativeVariable@ = $ExecutionContext.SessionState.PSVariable.Get('global:LASTEXITCODE')
        if ($null -eq $@VAR_nativeVariable@) {
            $global:LASTEXITCODE = $null
            $@VAR_nativeVariable@ = $ExecutionContext.SessionState.PSVariable.Get('global:LASTEXITCODE')
        }
        $@VAR_nativeVariable@.Attributes.Add($@VAR_nativeWrite@)
        $@VAR_nativeWrite@.Written = $false
        $@VAR_source@ = [IO.File]::ReadAllText($@VAR_scriptFile@, [Text.Encoding]::UTF8)
        $@VAR_userScript@ = [FuncTermScript]::Create(
            $@VAR_source@, '$@VAR_succeeded@ = $?; $@VAR_nativeCode@ = $global:LASTEXITCODE',
            '$@VAR_returned@ = $true'
        )
        $@VAR_pipeline@ = [PowerShell]::Create([System.Management.Automation.RunspaceMode]::CurrentRunspace)
        $null = $@VAR_pipeline@.AddScript('. $@VAR_userScript@', $false)
        if ($@VAR_SeparateOutput@) {
            $@VAR_capture@ = [FuncTermOutputCapture]::new($@VAR_Directory@)
            $@VAR_invocationSettings@ = [System.Management.Automation.PSInvocationSettings]::new()
            $@VAR_invocationSettings@.Host = [FuncTermDiagnosticHost]::new($Host)
            $@VAR_pipeline@.Commands.Commands[0].MergeMyResults(
                [System.Management.Automation.Runspaces.PipelineResultTypes]::Information,
                [System.Management.Automation.Runspaces.PipelineResultTypes]::Output
            )
            $null = $@VAR_pipeline@.AddCommand('Microsoft.PowerShell.Utility\Out-String').AddParameter('Stream')
            $null = $@VAR_pipeline@.AddCommand('Microsoft.PowerShell.Core\ForEach-Object').AddParameter('Process', { [Console]::WriteLine($_) })
        } else {
            $null = $@VAR_pipeline@.AddCommand('Microsoft.PowerShell.Core\Out-Default')
            $@VAR_pipeline@.Commands.Commands[-1].MergeUnclaimedPreviousCommandResults = (
                [System.Management.Automation.Runspaces.PipelineResultTypes]::Error -bor
                [System.Management.Automation.Runspaces.PipelineResultTypes]::Output
            )
        }
        $null = $@VAR_pipeline@.Invoke($null, $@VAR_invocationSettings@)
        if (-not $@VAR_returned@) {
            $@VAR_exitCode@ = [int]$global:LASTEXITCODE
            exit $@VAR_exitCode@
        }
        if ($@VAR_succeeded@) { $@VAR_exitCode@ = 0 }
        elseif ($@VAR_nativeWrite@.Written -and $null -ne $@VAR_nativeCode@ -and $@VAR_nativeCode@ -ne 0) {
            $@VAR_exitCode@ = [int]$@VAR_nativeCode@
        }
        else { $@VAR_exitCode@ = 1 }
    }
    catch {
        $@VAR_originalError@ = $_
        if ($_.Exception -is [System.Management.Automation.MethodInvocationException] -and
            $_.Exception.InnerException -is [System.Management.Automation.RuntimeException]) {
            $@VAR_originalError@ = $_.Exception.InnerException.ErrorRecord
            $global:Error.Remove($_)
            $global:Error.Insert(0, $@VAR_originalError@)
        }
        $@VAR_failure@ = Format-FuncTermError $@VAR_originalError@
        $@VAR_exitCode@ = 1
    }
    finally {
        $@VAR_timer@.Stop()
        if ($null -ne $@VAR_nativeVariable@) {
            $null = $@VAR_nativeVariable@.Attributes.Remove($@VAR_nativeWrite@)
        }
        if ($null -ne $@VAR_capture@) {
            Complete-FuncTermOutputCapture $@VAR_capture@
        }
        if ($null -ne $@VAR_pipeline@) {
            if ($@VAR_pipeline@.InvocationStateInfo.State -eq [System.Management.Automation.PSInvocationState]::Stopped) {
                $@VAR_exitCode@ = 130
            }
            foreach ($@VAR_error@ in $@VAR_pipeline@.Streams.Error) {
                $@VAR_formattedError@ = Format-FuncTermError $@VAR_error@
                [IO.File]::AppendAllText($@VAR_stderrFile@, $@VAR_formattedError@)
                [Console]::Error.Write($@VAR_formattedError@)
            }
            $@VAR_pipeline@.Dispose()
        }
        if ($null -ne $@VAR_failure@) {
            [IO.File]::AppendAllText($@VAR_stderrFile@, $@VAR_failure@ + [Environment]::NewLine)
            [Console]::Error.WriteLine($@VAR_failure@)
        }
        $@VAR_cleared@ = @(Get-ChildItem Env:).Count -eq 0
        foreach ($@VAR_entry@ in $@VAR_protectedEnvironment@.GetEnumerator()) {
            if ($@VAR_cleared@ -or $@VAR_entry@.Key -in @(@POWERSHELL_PROTECTED_ENVIRONMENT@)) {
                [Environment]::SetEnvironmentVariable($@VAR_entry@.Key, $@VAR_entry@.Value)
            }
        }
        Set-FuncTermShimPath
        $@VAR_elapsed@ = [string]::Format([Globalization.CultureInfo]::InvariantCulture, '{0}ms', $@VAR_timer@.Elapsed.TotalMilliseconds)
        $null = Invoke-FuncTermHelper -@VAR_Arguments@ @(
            'internal-write-done', '--command-id', $@VAR_CommandId@,
            '--exit-code', [string]$@VAR_exitCode@, '--time-consumption', $@VAR_elapsed@,
            '--cwd', $ExecutionContext.SessionState.Path.CurrentFileSystemLocation.ProviderPath,
            '--directory', $@VAR_Directory@
        )
        [Environment]::SetEnvironmentVariable('@COMMAND_ID_ENV@', $@VAR_previousCommandId@)
        [Environment]::SetEnvironmentVariable('@COMMAND_DIR_ENV@', $@VAR_previousCommandDirectory@)
    }
}
";
