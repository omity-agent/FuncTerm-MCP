use super::posix_dialect::PosixDialect;
use crate::contract::HELPER_EXECUTABLE_ENV;
pub (super) const POWERSHELL : & str = "function Publish-FuncTermStart {
	    param(
	        [Parameter(Mandatory = $true)][string]$@VAR_CommandId@,
	        [Parameter(Mandatory = $true)][string]$@VAR_Directory@
	    )
    if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
        throw '@HELPER_ENV@ is not set'
    }
	    $@VAR_startInfo@ = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
	    $@VAR_startInfo@.UseShellExecute = $false
	    $@VAR_startInfo@.ArgumentList.Add('internal-write-start')
	    $@VAR_startInfo@.ArgumentList.Add('--command-id')
	    $@VAR_startInfo@.ArgumentList.Add($@VAR_CommandId@)
	    $@VAR_startInfo@.ArgumentList.Add('--directory')
	    $@VAR_startInfo@.ArgumentList.Add($@VAR_Directory@)
	    $@VAR_startProcess@ = $null
	    try {
	        $@VAR_startProcess@ = [Diagnostics.Process]::Start($@VAR_startInfo@)
	        if ($null -eq $@VAR_startProcess@) {
	            throw 'command start helper did not start a process'
	        }
	        $@VAR_startProcess@.WaitForExit()
	        if ($@VAR_startProcess@.ExitCode -ne 0) {
	            throw ('command start helper failed with exit code {0}' -f $@VAR_startProcess@.ExitCode)
	        }
	    }
	    finally {
	        if ($null -ne $@VAR_startProcess@) {
	            $@VAR_startProcess@.Dispose()
	        }
    }
}" ;
pub (super) const POWERSHELL_SHIMS : & str = "function Set-FuncTermShimPath {
    if ([string]::IsNullOrEmpty($env:FUNCTERM_SHIM_DIR)) {
        return
    }
	    $@VAR_separator@ = [string][IO.Path]::PathSeparator
	    $@VAR_entries@ = $env:PATH -split [Regex]::Escape($@VAR_separator@)
	    $@VAR_remaining@ = $@VAR_entries@ | Where-Object {
	        -not [string]::Equals($_, $env:FUNCTERM_SHIM_DIR, [StringComparison]::OrdinalIgnoreCase)
	    }
	    $env:PATH = (@($env:FUNCTERM_SHIM_DIR) + @($@VAR_remaining@)) -join $@VAR_separator@
}
Set-FuncTermShimPath
function Ensure-FuncTermShims {
    if ([string]::IsNullOrEmpty($env:FUNCTERM_SHIM_DIR)) {
        return
    }
    if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
        throw '@HELPER_ENV@ is not set'
    }
	    $@VAR_shimStart@ = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
	    $@VAR_shimStart@.UseShellExecute = $false
	    $@VAR_shimStart@.RedirectStandardOutput = $true
	    $@VAR_shimStart@.RedirectStandardError = $true
	    $@VAR_shimStart@.ArgumentList.Add('internal-ensure-shims')
	    $@VAR_shimStart@.ArgumentList.Add('--directory')
	    $@VAR_shimStart@.ArgumentList.Add($env:FUNCTERM_SHIM_DIR)
	    try {
	        $@VAR_shimProcess@ = [Diagnostics.Process]::Start($@VAR_shimStart@)
    }
    catch {
        throw ('failed to start FuncTerm shell shim helper (helper: {0}; shim directory: {1}): {2}' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR, $_.Exception.Message)
    }
	    if ($null -eq $@VAR_shimProcess@) {
        throw ('FuncTerm shell shim helper did not start a process (helper: {0}; shim directory: {1})' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR)
	    }
	    try {
	        $@VAR_shimStdoutRead@ = $@VAR_shimProcess@.StandardOutput.ReadToEndAsync()
	        $@VAR_shimStderrRead@ = $@VAR_shimProcess@.StandardError.ReadToEndAsync()
	        $@VAR_shimProcess@.WaitForExit()
	        $@VAR_shimStdout@ = $@VAR_shimStdoutRead@.GetAwaiter().GetResult().Trim()
	        $@VAR_shimStderr@ = $@VAR_shimStderrRead@.GetAwaiter().GetResult().Trim()
	        $@VAR_shimExitCode@ = $@VAR_shimProcess@.ExitCode
    }
    catch {
        throw ('failed while waiting for FuncTerm shell shim helper (helper: {0}; shim directory: {1}): {2}' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR, $_.Exception.Message)
	    }
	    finally {
	        $@VAR_shimProcess@.Dispose()
	    }
	    if ($@VAR_shimExitCode@ -ne 0) {
	        $@VAR_shimDetails@ = @()
	        if (-not [string]::IsNullOrEmpty($@VAR_shimStderr@)) {
	            $@VAR_shimDetails@ += 'stderr: ' + $@VAR_shimStderr@
	        }
	        if (-not [string]::IsNullOrEmpty($@VAR_shimStdout@)) {
	            $@VAR_shimDetails@ += 'stdout: ' + $@VAR_shimStdout@
	        }
	        if ($@VAR_shimDetails@.Count -eq 0) {
	            $@VAR_shimDetails@ += 'no stdout or stderr output'
	        }
	        throw ('failed to ensure FuncTerm shell shims (helper: {0}; shim directory: {1}; exit code {2}): {3}' -f $env:@HELPER_ENV@, $env:FUNCTERM_SHIM_DIR, $@VAR_shimExitCode@, ($@VAR_shimDetails@ -join [Environment]::NewLine))
    }
}" ;
pub(super) fn posix(dialect: PosixDialect) -> String {
    format!(
        r#"functerm_publish_start() {{
{emulate}    local @VAR_command_id@="$1"
	    local @VAR_native_directory@="$2"
	    local @VAR_helper@="${{{helper_env}-}}"
	    if [ -z "$@VAR_helper@" ]; then
	        printf '%s is not set\n' "{helper_env}" >&2
	        return 1
	    fi
	    @VAR_helper@="$(functerm_posix_path "$@VAR_helper@")" || return 1
	    "$@VAR_helper@" internal-write-start \
	        --command-id "$@VAR_command_id@" \
	        --directory "$@VAR_native_directory@"
	}}"#,
        emulate = dialect.emulate(),
        helper_env = HELPER_EXECUTABLE_ENV,
    )
}
