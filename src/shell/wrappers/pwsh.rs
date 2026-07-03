use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, COMMAND_PAYLOAD_FILE, DONE_FILE, DONE_TEMP_FILE,
    HELPER_EXECUTABLE_ENV, POWERSHELL_COMMAND_FUNCTION, STARTED_FILE, STDERR_FILE, STDOUT_FILE,
};
pub(in crate::shell) fn wrapper() -> String {
    substitute(
        TEMPLATE,
        &[
            ("@COMMAND_DIR_ENV@", COMMAND_DIRECTORY_ENV),
            ("@COMMAND_ID_ENV@", COMMAND_ID_ENV),
            ("@DONE@", DONE_FILE),
            ("@DONE_TEMP@", DONE_TEMP_FILE),
            ("@FUNCTION@", POWERSHELL_COMMAND_FUNCTION),
            ("@HELPER_ENV@", HELPER_EXECUTABLE_ENV),
            ("@PAYLOAD@", COMMAND_PAYLOAD_FILE),
            ("@STDERR@", STDERR_FILE),
            ("@STARTED@", STARTED_FILE),
            ("@STDOUT@", STDOUT_FILE),
        ],
    )
}
fn substitute(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for &(placeholder, value) in pairs {
        text = text.replace(placeholder, value);
    }
    text
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
    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    $stdoutFile = Join-Path $Directory '@STDOUT@'
    $stderrFile = Join-Path $Directory '@STDERR@'
    $startedFile = Join-Path $Directory '@STARTED@'
    $payloadFile = Join-Path $Directory '@PAYLOAD@'
    $doneFile = Join-Path $Directory '@DONE@'
    $doneTempFile = Join-Path $Directory '@DONE_TEMP@'
    $previousCommandId = $env:@COMMAND_ID_ENV@
    $previousCommandDirectory = $env:@COMMAND_DIR_ENV@
    $env:@COMMAND_ID_ENV@ = $CommandId
    $env:@COMMAND_DIR_ENV@ = $Directory
    try {
        Set-Location -LiteralPath $WorkingDirectory
        $global:LASTEXITCODE = $null
        $Payload = Get-Content -LiteralPath $payloadFile -Raw -Encoding utf8
        $script = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Payload))
        Set-Content -LiteralPath $startedFile -Value '' -NoNewline -Encoding utf8
        & ([scriptblock]::Create($script)) 2> $stderrFile | Tee-Object -FilePath $stdoutFile
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
    if (-not (Test-Path -LiteralPath $doneFile)) {
        New-Item -ItemType Directory -Force -Path $Directory | Out-Null
        if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
            [Console]::Error.WriteLine('@HELPER_ENV@ is not set')
            $exitCode = 1
        }
        else {
            & $env:@HELPER_ENV@ internal-write-done --command-id $CommandId --exit-code $exitCode --cwd (Get-Location).Path --directory $Directory
            if ($LASTEXITCODE -ne 0) {
                $exitCode = $LASTEXITCODE
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
" ;
