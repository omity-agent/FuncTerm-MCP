use super::posix_dialect::PosixDialect;
use crate::contract::HELPER_EXECUTABLE_ENV;
pub(super) const POWERSHELL: &str = "function Publish-FuncTermStart {
    param(
        [Parameter(Mandatory = $true)][string]$CommandId,
        [Parameter(Mandatory = $true)][string]$Directory
    )
    if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
        throw '@HELPER_ENV@ is not set'
    }
    $startInfo = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
    $startInfo.UseShellExecute = $false
    $startInfo.ArgumentList.Add('internal-write-start')
    $startInfo.ArgumentList.Add('--command-id')
    $startInfo.ArgumentList.Add($CommandId)
    $startInfo.ArgumentList.Add('--directory')
    $startInfo.ArgumentList.Add($Directory)
    $startProcess = $null
    try {
        $startProcess = [Diagnostics.Process]::Start($startInfo)
        if ($null -eq $startProcess) {
            throw 'command start helper did not start a process'
        }
        $startProcess.WaitForExit()
        if ($startProcess.ExitCode -ne 0) {
            throw ('command start helper failed with exit code {0}' -f $startProcess.ExitCode)
        }
    }
    finally {
        if ($null -ne $startProcess) {
            $startProcess.Dispose()
        }
    }
}";
pub (super) const POWERSHELL_SHIMS : & str = "function Set-FuncTermShimPath {
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
    $shimStart = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
    $shimStart.UseShellExecute = $false
    $shimStart.RedirectStandardOutput = $true
    $shimStart.RedirectStandardError = $true
    $shimStart.ArgumentList.Add('internal-ensure-shims')
    $shimStart.ArgumentList.Add('--directory')
    $shimStart.ArgumentList.Add($env:FUNCTERM_SHIM_DIR)
    try {
        $shimProcess = [Diagnostics.Process]::Start($shimStart)
    }
    catch {
        throw ('failed to start FuncTerm shell shim helper (helper: {0}; shim directory: {1}): {2}' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR, $_.Exception.Message)
    }
    if ($null -eq $shimProcess) {
        throw ('FuncTerm shell shim helper did not start a process (helper: {0}; shim directory: {1})' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR)
    }
    try {
        $shimStdoutRead = $shimProcess.StandardOutput.ReadToEndAsync()
        $shimStderrRead = $shimProcess.StandardError.ReadToEndAsync()
        $shimProcess.WaitForExit()
        $shimStdout = $shimStdoutRead.GetAwaiter().GetResult().Trim()
        $shimStderr = $shimStderrRead.GetAwaiter().GetResult().Trim()
        $shimExitCode = $shimProcess.ExitCode
    }
    catch {
        throw ('failed while waiting for FuncTerm shell shim helper (helper: {0}; shim directory: {1}): {2}' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR, $_.Exception.Message)
    }
    finally {
        $shimProcess.Dispose()
    }
    if ($shimExitCode -ne 0) {
        $shimDetails = @()
        if (-not [string]::IsNullOrEmpty($shimStderr)) {
            $shimDetails += 'stderr: ' + $shimStderr
        }
        if (-not [string]::IsNullOrEmpty($shimStdout)) {
            $shimDetails += 'stdout: ' + $shimStdout
        }
        if ($shimDetails.Count -eq 0) {
            $shimDetails += 'no stdout or stderr output'
        }
        throw ('failed to ensure FuncTerm shell shims (helper: {0}; shim directory: {1}; exit code {2}): {3}' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR, $shimExitCode, ($shimDetails -join [Environment]::NewLine))
    }
}" ;
pub(super) fn posix(dialect: PosixDialect) -> String {
    format!(
        r#"functerm_publish_start() {{
{emulate}    local command_id="$1"
    local native_directory="$2"
    local helper="${{{helper_env}-}}"
    if [ -z "$helper" ]; then
        printf '%s is not set\n' "{helper_env}" >&2
        return 1
    fi
    helper="$(functerm_posix_path "$helper")" || return 1
    "$helper" internal-write-start \
        --command-id "$command_id" \
        --directory "$native_directory"
}}"#,
        emulate = dialect.emulate(),
        helper_env = HELPER_EXECUTABLE_ENV,
    )
}
