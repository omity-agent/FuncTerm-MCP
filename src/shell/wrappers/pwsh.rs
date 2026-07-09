use super::template;
use crate::contract::POWERSHELL_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    template::render_command_function(TEMPLATE, POWERSHELL_COMMAND_FUNCTION)
}
const TEMPLATE : & str = "function Set-FuncTermShimPath {
    if ([string]::IsNullOrEmpty($env:FUNCTERM_SHIM_DIR)) {
        return
    }
    $separator = [string][IO.Path]::PathSeparator
    $entries = $env:PATH -split [Regex]::Escape($separator)
    $remaining = $entries | Where-Object {
        -not [string]::Equals($_, $env:FUNCTERM_SHIM_DIR, [StringComparison]::OrdinalIgnoreCase)
    }
    $env:PATH = (@($env:FUNCTERM_SHIM_DIR) + @($remaining)) -join $separator
}
Set-FuncTermShimPath
function Ensure-FuncTermShims {
    if ([string]::IsNullOrEmpty($env:FUNCTERM_SHIM_DIR)) {
        return
    }
    if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
        throw '@HELPER_ENV@ is not set'
    }
    & $env:@HELPER_ENV@ internal-ensure-shims --directory $env:FUNCTERM_SHIM_DIR
    if ($LASTEXITCODE -ne 0) {
        throw 'failed to ensure FuncTerm shell shims'
    }
}
if (Get-Command Set-PSReadLineOption -ErrorAction SilentlyContinue) {
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
    $startedFile = Join-Path $stateDir '@STARTED@'
    $commandFile = Join-Path $inputDir '@COMMAND@'
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
        $script = Get-Content -LiteralPath $commandFile -Raw -Encoding utf8
        Set-Content -LiteralPath $startedFile -Value '' -NoNewline -Encoding utf8
        $commandTimer = [Diagnostics.Stopwatch]::StartNew()
        & ([scriptblock]::Create($script)) 2> $stderrFile | Tee-Object -FilePath $stdoutFile
        $commandTimer.Stop()
        $timeConsumption = [string]::Format(
            [Globalization.CultureInfo]::InvariantCulture,
            '{0}ms',
            $commandTimer.Elapsed.TotalMilliseconds
        )
        if ($null -ne $global:LASTEXITCODE) {
            $exitCode = [int]$global:LASTEXITCODE
        }
        elseif ($?) {
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
