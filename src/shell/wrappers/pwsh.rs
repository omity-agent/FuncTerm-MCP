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
    template::render_powershell(&rendered.replace(
        "@POWERSHELL_STATE_PROMOTION@",
        template::POWERSHELL_STATE_PROMOTION,
    ))
}
const COMMAND_TEMPLATE : & str = "if (Get-Command Set-PSReadLineOption -ErrorAction SilentlyContinue) {
    Set-PSReadLineOption -HistorySaveStyle SaveNothing
    $setPsReadLineOption = Get-Command Set-PSReadLineOption
    if ($setPsReadLineOption.Parameters.ContainsKey('AddToHistoryHandler')) {
        Set-PSReadLineOption -AddToHistoryHandler {
            param([string] $line)
            return $false
        }
    }
}
Clear-History
function @FUNCTION@ {
    param(
        [Parameter(Mandatory = $true)][string]$CommandId,
        [Parameter(Mandatory = $true)][string]$Directory,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory
    )
    Set-FuncTermShimPath
    $inputDir = Join-Path $Directory '@INPUT_DIR@'
    $outputDir = Join-Path $Directory '@OUTPUT_DIR@'
    $stateDir = Join-Path $Directory '@STATE_DIR@'
    $stdoutFile = Join-Path $outputDir '@STDOUT@'
    $stderrFile = Join-Path $outputDir '@STDERR@'
    $scriptFile = Join-Path $inputDir '@SCRIPT@'
    $doneFile = Join-Path $stateDir '@DONE@'
    $previousCommandId = $env:@COMMAND_ID_ENV@
    $previousCommandDirectory = $env:@COMMAND_DIR_ENV@
    $exitCode = 1
    $timeConsumption = '0ns'
    trap {
        [IO.File]::AppendAllText($stderrFile, [string]$_ + [Environment]::NewLine, [Text.Encoding]::UTF8)
        [Console]::Error.WriteLine($_)
        $exitCode = 130
        continue
    }
    $env:@COMMAND_ID_ENV@ = $CommandId
    $env:@COMMAND_DIR_ENV@ = $Directory
    try {
        Ensure-FuncTermShims
        Set-FuncTermShimPath
        Set-Location -LiteralPath $WorkingDirectory
        $global:LASTEXITCODE = $null
        Publish-FuncTermStart -CommandId $CommandId -Directory $Directory
        $global:LASTEXITCODE = $null
        $script:FuncTermCommandNativeExitCode = $null
        $script:FuncTermCommandSucceeded = $false
        $existingVariables = $null
        $existingFunctions = $null
        $existingAliases = $null
        $commandTimer = $null
        $variable = $null
        $function = $null
        $alias = $null
        $existingVariables = (Get-Variable -Scope Local).Name
        $existingFunctions = @{}
        foreach ($function in Get-ChildItem Function:) {
            $existingFunctions[$function.Name] = $function.ScriptBlock.ToString()
        }
        $existingAliases = @{}
        foreach ($alias in Get-ChildItem Alias:) {
            $existingAliases[$alias.Name] = $alias.Definition
        }
        $commandTimer = [Diagnostics.Stopwatch]::StartNew()
        . $scriptFile
        . $script:FuncTermCommandScript 2> $stderrFile | Tee-Object -FilePath $stdoutFile
        $script:FuncTermCommandSucceeded = $?
        $script:FuncTermCommandNativeExitCode = $global:LASTEXITCODE
        $commandTimer.Stop()
@POWERSHELL_STATE_PROMOTION@
        $timeConsumption = [string]::Format(
            [Globalization.CultureInfo]::InvariantCulture,
            '{0}ms',
            $commandTimer.Elapsed.TotalMilliseconds
        )
        if ($null -ne $script:FuncTermCommandNativeExitCode) {
            $exitCode = [int]$script:FuncTermCommandNativeExitCode
        }
        elseif ($script:FuncTermCommandSucceeded) {
            $exitCode = 0
        }
        else {
            $exitCode = 1
        }
        if ((Test-Path -LiteralPath $stderrFile) -and ((Get-Item -LiteralPath $stderrFile).Length -gt 0)) {
            Get-Content -LiteralPath $stderrFile | ForEach-Object { [Console]::Error.WriteLine($_) }
        }
    }
    catch {
        $_ | Out-File -LiteralPath $stderrFile -Append -Encoding utf8
        [Console]::Error.WriteLine($_)
        $exitCode = 1
    }
    finally {
        if (-not [IO.File]::Exists($doneFile)) {
            $null = [IO.Directory]::CreateDirectory($stateDir)
            if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
                [Console]::Error.WriteLine('@HELPER_ENV@ is not set')
                $exitCode = 1
            }
            else {
                $currentDirectory = $ExecutionContext.SessionState.Path.CurrentFileSystemLocation.ProviderPath
                $helperStart = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
                $helperStart.UseShellExecute = $false
                $helperStart.ArgumentList.Add('internal-write-done')
                $helperStart.ArgumentList.Add('--command-id')
                $helperStart.ArgumentList.Add($CommandId)
                $helperStart.ArgumentList.Add('--exit-code')
                $helperStart.ArgumentList.Add([string]$exitCode)
                $helperStart.ArgumentList.Add('--time-consumption')
                $helperStart.ArgumentList.Add($timeConsumption)
                $helperStart.ArgumentList.Add('--cwd')
                $helperStart.ArgumentList.Add($currentDirectory)
                $helperStart.ArgumentList.Add('--directory')
                $helperStart.ArgumentList.Add($Directory)
                $helperProcess = [Diagnostics.Process]::Start($helperStart)
                $helperProcess.WaitForExit()
                if ($helperProcess.ExitCode -ne 0) {
                    $exitCode = $helperProcess.ExitCode
                }
            }
        }
        if ($null -eq $previousCommandId) {
            Remove-Item Env:@COMMAND_ID_ENV@ -ErrorAction SilentlyContinue
        }
        else {
            $env:@COMMAND_ID_ENV@ = $previousCommandId
        }
        if ($null -eq $previousCommandDirectory) {
            Remove-Item Env:@COMMAND_DIR_ENV@ -ErrorAction SilentlyContinue
        }
        else {
            $env:@COMMAND_DIR_ENV@ = $previousCommandDirectory
        }
    }
}
" ;
