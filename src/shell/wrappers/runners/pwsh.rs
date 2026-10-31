use super::super::{VariableNamespace, start, template, variables};
use crate::contract::POWERSHELL_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    let script = format!(
        "{}\n{}\n{}\n{COMMAND_TEMPLATE}\n{}",
        start::POWERSHELL,
        start::POWERSHELL_SHIMS,
        super::ordered_streams::SCRIPT,
        template::powershell_dispatcher()
    );
    let rendered = template::render_command_function(&script, POWERSHELL_COMMAND_FUNCTION);
    let protected = rendered.replace(
        "@POWERSHELL_PROTECTED_ENVIRONMENT@",
        &variables::powershell_protected_environment_names(),
    );
    VariableNamespace::new().render_powershell(&template::render_powershell(&protected))
}
const COMMAND_TEMPLATE: &str = r#"& {
    if (Get-Command Set-PSReadLineOption -ErrorAction SilentlyContinue) {
        Set-PSReadLineOption -HistorySaveStyle SaveNothing
        if ((Get-Command Set-PSReadLineOption).Parameters.ContainsKey('AddToHistoryHandler')) {
            Set-PSReadLineOption -AddToHistoryHandler { $false }
        }
    }
    Clear-History
}
function ConvertTo-FuncTermInvocation {
    param([string]$@VAR_source@)
    $@VAR_tokens@ = $null
    $@VAR_parseErrors@ = $null
    $@VAR_ast@ = [System.Management.Automation.Language.Parser]::ParseInput($@VAR_source@, [ref]$@VAR_tokens@, [ref]$@VAR_parseErrors@)
    if ($@VAR_parseErrors@.Count -ne 0) {
        throw [System.Management.Automation.ParseException]::new($@VAR_parseErrors@)
    }
    $@VAR_headerLength@ = 0
    foreach ($@VAR_header@ in @($@VAR_ast@.UsingStatements) + @($@VAR_ast@.ParamBlock)) {
        if ($null -ne $@VAR_header@) {
            $@VAR_headerLength@ = [Math]::Max($@VAR_headerLength@, $@VAR_header@.Extent.EndOffset)
        }
    }
    $@VAR_instrumented@ = $@VAR_source@
    $@VAR_finalizer@ = '
} finally {
$@VAR_succeeded@ = $?
$@VAR_nativeCode@ = $global:LASTEXITCODE
}
'
    if ($@VAR_ast@.EndBlock.Unnamed) {
        $@VAR_instrumented@ = $@VAR_source@.Insert($@VAR_source@.Length, $@VAR_finalizer@).Insert($@VAR_headerLength@, "`ntry {`n")
    } else {
        $@VAR_blocks@ = @($@VAR_ast@.DynamicParamBlock, $@VAR_ast@.BeginBlock, $@VAR_ast@.ProcessBlock, $@VAR_ast@.EndBlock, $@VAR_ast@.CleanBlock)
        foreach ($@VAR_block@ in $@VAR_blocks@ | Where-Object { $null -ne $_ } | Sort-Object { $_.Extent.StartOffset } -Descending) {
            $@VAR_opening@ = $@VAR_tokens@ | Where-Object { $_.Kind -eq 'LCurly' -and $_.Extent.StartOffset -ge $@VAR_block@.Extent.StartOffset } | Select-Object -First 1
            $@VAR_instrumented@ = $@VAR_instrumented@.Insert($@VAR_block@.Extent.EndOffset - 1, $@VAR_finalizer@).Insert($@VAR_opening@.Extent.EndOffset, "`ntry {`n")
        }
    }
    return $@VAR_instrumented@.Insert($@VAR_headerLength@, "`n. {`n") + '
}
$@VAR_returned@ = $true'
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
    $@VAR_timer@ = [Diagnostics.Stopwatch]::new()
    $env:@COMMAND_ID_ENV@ = $@VAR_CommandId@
    $env:@COMMAND_DIR_ENV@ = $@VAR_Directory@
    try {
        Set-FuncTermShimPath
        Set-Location -LiteralPath $@VAR_WorkingDirectory@
        Publish-FuncTermStart -@VAR_CommandId@ $@VAR_CommandId@ -@VAR_Directory@ $@VAR_Directory@
        $global:LASTEXITCODE = $null
        $@VAR_succeeded@ = $false
        $@VAR_nativeCode@ = $null
        $@VAR_returned@ = $false
        $@VAR_timer@.Start()
        $@VAR_source@ = [IO.File]::ReadAllText($@VAR_scriptFile@, [Text.Encoding]::UTF8)
        $@VAR_pipeline@ = [PowerShell]::Create([System.Management.Automation.RunspaceMode]::CurrentRunspace)
        $null = $@VAR_pipeline@.AddScript((ConvertTo-FuncTermInvocation $@VAR_source@), $false)
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
        elseif ($null -ne $@VAR_nativeCode@ -and $@VAR_nativeCode@ -ne 0) {
            $@VAR_exitCode@ = [int]$@VAR_nativeCode@
        }
        else { $@VAR_exitCode@ = 1 }
    }
    catch {
        $@VAR_failure@ = [string]$_
        $@VAR_exitCode@ = 1
    }
    finally {
        $@VAR_timer@.Stop()
        if ($null -ne $@VAR_capture@) {
            Complete-FuncTermOutputCapture $@VAR_capture@
        }
        if ($null -ne $@VAR_pipeline@) {
            if ($@VAR_pipeline@.InvocationStateInfo.State -eq [System.Management.Automation.PSInvocationState]::Stopped) {
                $@VAR_exitCode@ = 130
            }
            foreach ($@VAR_error@ in $@VAR_pipeline@.Streams.Error) {
                [IO.File]::AppendAllText($@VAR_stderrFile@, [string]$@VAR_error@ + [Environment]::NewLine)
                [Console]::Error.WriteLine($@VAR_error@)
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
"#;
