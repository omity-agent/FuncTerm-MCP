use super::template;
use crate::contract::POWERSHELL_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    let script = format!(
        "{}\n{}\n{COMMAND_TEMPLATE}\n{}",
        super::start::POWERSHELL,
        super::start::POWERSHELL_SHIMS,
        super::template::powershell_dispatcher()
    );
    let rendered = template::render_command_function(&script, POWERSHELL_COMMAND_FUNCTION);
    let promoted = rendered.replace(
        "@POWERSHELL_STATE_PROMOTION@",
        template::POWERSHELL_STATE_PROMOTION,
    );
    let protected = promoted.replace(
        "@POWERSHELL_PROTECTED_ENVIRONMENT@",
        &super::variables::powershell_protected_environment_names(),
    );
    let wrapper = template::render_powershell(&protected);
    super::VariableNamespace::new().render(&wrapper)
}
const COMMAND_TEMPLATE : & str = "if (Get-Command Set-PSReadLineOption -ErrorAction SilentlyContinue) {
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
function @FUNCTION@ {
    param(
        [Parameter(Mandatory = $true)][string]$@VAR_CommandId@,
        [Parameter(Mandatory = $true)][string]$@VAR_Directory@,
        [Parameter(Mandatory = $true)][string]$@VAR_WorkingDirectory@
    )
    Set-FuncTermShimPath
    $@VAR_inputDir@ = Join-Path $@VAR_Directory@ '@INPUT_DIR@'
    $@VAR_outputDir@ = Join-Path $@VAR_Directory@ '@OUTPUT_DIR@'
    $@VAR_stateDir@ = Join-Path $@VAR_Directory@ '@STATE_DIR@'
    $@VAR_stdoutFile@ = Join-Path $@VAR_outputDir@ '@STDOUT@'
    $@VAR_stderrFile@ = Join-Path $@VAR_outputDir@ '@STDERR@'
    $@VAR_scriptFile@ = Join-Path $@VAR_inputDir@ '@SCRIPT@'
    $@VAR_doneFile@ = Join-Path $@VAR_stateDir@ '@DONE@'
    $@VAR_previousCommandId@ = $env:@COMMAND_ID_ENV@
    $@VAR_previousCommandDirectory@ = $env:@COMMAND_DIR_ENV@
    $@VAR_protectedEnvironment@ = [Collections.Generic.Dictionary[string, string]]::new(
        [StringComparer]::OrdinalIgnoreCase
    )
    foreach ($@VAR_environmentEntry@ in Get-ChildItem Env:) {
        $@VAR_protectedEnvironment@[$@VAR_environmentEntry@.Name] = $@VAR_environmentEntry@.Value
    }
    $@VAR_exitCode@ = 1
    $@VAR_timeConsumption@ = '0ns'
    trap {
        [IO.File]::AppendAllText($@VAR_stderrFile@, [string]$_ + [Environment]::NewLine, [Text.Encoding]::UTF8)
        [Console]::Error.WriteLine($_)
        $@VAR_exitCode@ = 130
        continue
    }
    $env:@COMMAND_ID_ENV@ = $@VAR_CommandId@
    $env:@COMMAND_DIR_ENV@ = $@VAR_Directory@
    try {
        Ensure-FuncTermShims
        Set-FuncTermShimPath
        Set-Location -LiteralPath $@VAR_WorkingDirectory@
        $global:LASTEXITCODE = $null
        Publish-FuncTermStart -@VAR_CommandId@ $@VAR_CommandId@ -@VAR_Directory@ $@VAR_Directory@
        $global:LASTEXITCODE = $null
        $@VAR_commandNativeExitCode@ = $null
        $@VAR_commandSucceeded@ = $false
        $@VAR_existingVariables@ = $null
        $@VAR_existingFunctions@ = $null
        $@VAR_existingAliases@ = $null
        $@VAR_commandTimer@ = $null
        $@VAR_commandScript@ = $null
        $@VAR_variable@ = $null
        $@VAR_function@ = $null
        $@VAR_alias@ = $null
        $@VAR_existingVariables@ = (Get-Variable -Scope Local).Name
        $@VAR_existingFunctions@ = @{}
        foreach ($@VAR_function@ in Get-ChildItem Function:) {
            $@VAR_existingFunctions@[$@VAR_function@.Name] = $@VAR_function@.ScriptBlock.ToString()
        }
        $@VAR_existingAliases@ = @{}
        foreach ($@VAR_alias@ in Get-ChildItem Alias:) {
            $@VAR_existingAliases@[$@VAR_alias@.Name] = $@VAR_alias@.Definition
        }
        $@VAR_commandScript@ = [scriptblock]::Create(
            [IO.File]::ReadAllText($@VAR_scriptFile@, [Text.Encoding]::UTF8)
        )
        $@VAR_commandTimer@ = [Diagnostics.Stopwatch]::StartNew()
        . $@VAR_commandScript@ 2> $@VAR_stderrFile@ | Tee-Object -FilePath $@VAR_stdoutFile@
        $@VAR_commandSucceeded@ = $?
        $@VAR_commandNativeExitCode@ = $global:LASTEXITCODE
        $@VAR_commandTimer@.Stop()
@POWERSHELL_STATE_PROMOTION@
        $@VAR_timeConsumption@ = [string]::Format(
            [Globalization.CultureInfo]::InvariantCulture,
            '{0}ms',
            $@VAR_commandTimer@.Elapsed.TotalMilliseconds
        )
        if ($null -ne $@VAR_commandNativeExitCode@) {
            $@VAR_exitCode@ = [int]$@VAR_commandNativeExitCode@
        }
        elseif ($@VAR_commandSucceeded@) {
            $@VAR_exitCode@ = 0
        }
        else {
            $@VAR_exitCode@ = 1
        }
        if ((Test-Path -LiteralPath $@VAR_stderrFile@) -and ((Get-Item -LiteralPath $@VAR_stderrFile@).Length -gt 0)) {
            Get-Content -LiteralPath $@VAR_stderrFile@ | ForEach-Object { [Console]::Error.WriteLine($_) }
        }
    }
    catch {
        $_ | Out-File -LiteralPath $@VAR_stderrFile@ -Append -Encoding utf8
        [Console]::Error.WriteLine($_)
        $@VAR_exitCode@ = 1
    }
    finally {
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
        if (-not [IO.File]::Exists($@VAR_doneFile@)) {
            $null = [IO.Directory]::CreateDirectory($@VAR_stateDir@)
            if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
                [Console]::Error.WriteLine('@HELPER_ENV@ is not set')
                $@VAR_exitCode@ = 1
            }
            else {
                $@VAR_currentDirectory@ = $ExecutionContext.SessionState.Path.CurrentFileSystemLocation.ProviderPath
                $@VAR_helperStart@ = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
                $@VAR_helperStart@.UseShellExecute = $false
                $@VAR_helperStart@.ArgumentList.Add('internal-write-done')
                $@VAR_helperStart@.ArgumentList.Add('--command-id')
                $@VAR_helperStart@.ArgumentList.Add($@VAR_CommandId@)
                $@VAR_helperStart@.ArgumentList.Add('--exit-code')
                $@VAR_helperStart@.ArgumentList.Add([string]$@VAR_exitCode@)
                $@VAR_helperStart@.ArgumentList.Add('--time-consumption')
                $@VAR_helperStart@.ArgumentList.Add($@VAR_timeConsumption@)
                $@VAR_helperStart@.ArgumentList.Add('--cwd')
                $@VAR_helperStart@.ArgumentList.Add($@VAR_currentDirectory@)
                $@VAR_helperStart@.ArgumentList.Add('--directory')
                $@VAR_helperStart@.ArgumentList.Add($@VAR_Directory@)
                $@VAR_helperProcess@ = [Diagnostics.Process]::Start($@VAR_helperStart@)
                $@VAR_helperProcess@.WaitForExit()
                if ($@VAR_helperProcess@.ExitCode -ne 0) {
                    $@VAR_exitCode@ = $@VAR_helperProcess@.ExitCode
                }
            }
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
" ;
