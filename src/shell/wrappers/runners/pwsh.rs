use super::super::{VariableNamespace, start, template, variables};
use crate::contract::POWERSHELL_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    let script = format!(
        "{}\n{}\n{COMMAND_TEMPLATE}\n{}",
        start::POWERSHELL,
        start::POWERSHELL_SHIMS,
        template::powershell_dispatcher()
    );
    let rendered = template::render_command_function(&script, POWERSHELL_COMMAND_FUNCTION);
    let protected = rendered.replace(
        "@POWERSHELL_PROTECTED_ENVIRONMENT@",
        &variables::powershell_protected_environment_names(),
    );
    let wrapper = template::render_powershell(&protected);
    VariableNamespace::new().render_powershell(&wrapper)
}
const COMMAND_TEMPLATE: &str = r"& {
    if (Get-Command Set-PSReadLineOption -ErrorAction SilentlyContinue) {
        Set-PSReadLineOption -HistorySaveStyle SaveNothing
        $@VAR_setPsReadLineOption@ = Get-Command Set-PSReadLineOption
        if ($@VAR_setPsReadLineOption@.Parameters.ContainsKey('AddToHistoryHandler')) {
            Set-PSReadLineOption -AddToHistoryHandler {
                param([string] $@VAR_line@)
                return $false
            }
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
    Set-FuncTermShimPath
    $@VAR_inputDir@ = Join-Path $@VAR_Directory@ '@INPUT_DIR@'
    $@VAR_outputDir@ = Join-Path $@VAR_Directory@ '@OUTPUT_DIR@'
    $@VAR_stdoutFile@ = Join-Path $@VAR_outputDir@ '@STDOUT@'
    $@VAR_stderrFile@ = Join-Path $@VAR_outputDir@ '@STDERR@'
    $@VAR_scriptFile@ = Join-Path $@VAR_inputDir@ '@SCRIPT@'
    $@VAR_previousCommandId@ = $env:@COMMAND_ID_ENV@
    $@VAR_previousCommandDirectory@ = $env:@COMMAND_DIR_ENV@
    $@VAR_protectedEnvironment@ = [Collections.Generic.Dictionary[string, string]]::new(
        [StringComparer]::OrdinalIgnoreCase
    )
    foreach ($@VAR_environmentEntry@ in Get-ChildItem Env:) {
        $@VAR_protectedEnvironment@[$@VAR_environmentEntry@.Name] = $@VAR_environmentEntry@.Value
    }
    $@VAR_exitCode@ = 1
    $@VAR_commandFailure@ = $null
    $@VAR_commandPipeline@ = $null
    $@VAR_commandTimer@ = [Diagnostics.Stopwatch]::new()
    trap [System.Management.Automation.PipelineStoppedException] {
        [IO.File]::AppendAllText($@VAR_stderrFile@, [string]$_ + [Environment]::NewLine)
        [Console]::Error.WriteLine($_)
        $@VAR_exitCode@ = 130
        continue
    }
    $env:@COMMAND_ID_ENV@ = $@VAR_CommandId@
    $env:@COMMAND_DIR_ENV@ = $@VAR_Directory@
    try {
        Set-FuncTermShimPath
        Set-Location -LiteralPath $@VAR_WorkingDirectory@
        $global:LASTEXITCODE = $null
        Publish-FuncTermStart -@VAR_CommandId@ $@VAR_CommandId@ -@VAR_Directory@ $@VAR_Directory@
        $global:LASTEXITCODE = $null
        $@VAR_commandNativeExitCode@ = $null
        $@VAR_commandSucceeded@ = $false
        $@VAR_commandReturned@ = $false
        $@VAR_commandTimer@.Start()
        $@VAR_commandScript@ = [scriptblock]::Create(
            [IO.File]::ReadAllText($@VAR_scriptFile@, [Text.Encoding]::UTF8)
        )
        $@VAR_commandPipeline@ = [PowerShell]::Create(
            [System.Management.Automation.RunspaceMode]::CurrentRunspace
        )
        $null = $@VAR_commandPipeline@.AddScript(
            '. $@VAR_commandScript@
$@VAR_commandSucceeded@ = $?
$@VAR_commandNativeExitCode@ = $global:LASTEXITCODE
$@VAR_commandReturned@ = $true',
            $false
        )
        if ($@VAR_SeparateOutput@) {
            [IO.File]::WriteAllText($@VAR_stderrFile@, '')
            $null = $@VAR_commandPipeline@.AddCommand('Microsoft.PowerShell.Utility\Tee-Object').AddParameter(
                'FilePath', $@VAR_stdoutFile@
            )
        }
        $null = $@VAR_commandPipeline@.AddCommand('Microsoft.PowerShell.Core\Out-Default')
        if (-not $@VAR_SeparateOutput@) {
            $@VAR_commandPipeline@.Commands.Commands[-1].MergeUnclaimedPreviousCommandResults = (
                [System.Management.Automation.Runspaces.PipelineResultTypes]::Error -bor
                [System.Management.Automation.Runspaces.PipelineResultTypes]::Output
            )
        }
        $null = $@VAR_commandPipeline@.Invoke()
        if (-not $@VAR_commandReturned@) {
            $@VAR_exitCode@ = [int]$global:LASTEXITCODE
            exit $@VAR_exitCode@
        }
        if ($null -ne $@VAR_commandNativeExitCode@) {
            $@VAR_exitCode@ = [int]$@VAR_commandNativeExitCode@
        }
        elseif ($@VAR_commandSucceeded@) {
            $@VAR_exitCode@ = 0
        }
        else {
            $@VAR_exitCode@ = 1
        }
    }
    catch [System.Management.Automation.ParseException] {
        $@VAR_commandFailure@ = $_.Exception.Message
        $@VAR_exitCode@ = 1
    }
    catch {
        if ($null -ne $@VAR_commandPipeline@ -and $null -ne $@VAR_commandPipeline@.InvocationStateInfo.Reason) {
            $@VAR_commandFailure@ = $@VAR_commandPipeline@.InvocationStateInfo.Reason.Message
        }
        else {
            $@VAR_commandFailure@ = [string]$_
        }
        $@VAR_exitCode@ = 1
    }
    finally {
        $@VAR_commandTimer@.Stop()
        $@VAR_timeConsumption@ = [string]::Format(
            [Globalization.CultureInfo]::InvariantCulture,
            '{0}ms',
            $@VAR_commandTimer@.Elapsed.TotalMilliseconds
        )
        if ($null -ne $@VAR_commandPipeline@) {
            if ($@VAR_commandPipeline@.InvocationStateInfo.State -eq [System.Management.Automation.PSInvocationState]::Stopped) {
                $@VAR_exitCode@ = 130
            }
            foreach ($@VAR_error@ in $@VAR_commandPipeline@.Streams.Error) {
                [IO.File]::AppendAllText($@VAR_stderrFile@, [string]$@VAR_error@ + [Environment]::NewLine)
                [Console]::Error.WriteLine($@VAR_error@)
            }
            $@VAR_commandPipeline@.Dispose()
        }
        if ($null -ne $@VAR_commandFailure@) {
            [IO.File]::AppendAllText($@VAR_stderrFile@, $@VAR_commandFailure@ + [Environment]::NewLine)
            [Console]::Error.WriteLine($@VAR_commandFailure@)
        }
        $@VAR_environmentWasCleared@ = @(Get-ChildItem Env:).Count -eq 0
        foreach ($@VAR_environmentEntry@ in $@VAR_protectedEnvironment@.GetEnumerator()) {
            if (
                $@VAR_environmentWasCleared@ -or
                $@VAR_environmentEntry@.Key -in @(@POWERSHELL_PROTECTED_ENVIRONMENT@)
            ) {
                [Environment]::SetEnvironmentVariable(
                    $@VAR_environmentEntry@.Key,
                    $@VAR_environmentEntry@.Value
                )
            }
        }
        Set-FuncTermShimPath
        if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
            [Console]::Error.WriteLine('@HELPER_ENV@ is not set')
            $@VAR_exitCode@ = 1
        }
        else {
            $@VAR_currentDirectory@ = $ExecutionContext.SessionState.Path.CurrentFileSystemLocation.ProviderPath
            $null = Invoke-FuncTermHelper -@VAR_Arguments@ @(
                'internal-write-done', '--command-id', $@VAR_CommandId@,
                '--exit-code', [string]$@VAR_exitCode@,
                '--time-consumption', $@VAR_timeConsumption@,
                '--cwd', $@VAR_currentDirectory@, '--directory', $@VAR_Directory@
            )
        }
        if ($null -eq $@VAR_previousCommandId@) {
            Remove-Item Env:@COMMAND_ID_ENV@ -ErrorAction SilentlyContinue
        }
        else {
            $env:@COMMAND_ID_ENV@ = $@VAR_previousCommandId@
        }
        if ($null -eq $@VAR_previousCommandDirectory@) {
            Remove-Item Env:@COMMAND_DIR_ENV@ -ErrorAction SilentlyContinue
        }
        else {
            $env:@COMMAND_DIR_ENV@ = $@VAR_previousCommandDirectory@
        }
    }
}
";
