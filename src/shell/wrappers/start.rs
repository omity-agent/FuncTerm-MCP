use super::posix::PosixDialect;
use crate::contract::HELPER_EXECUTABLE_ENV;
pub (super) const POWERSHELL : & str = "function Invoke-FuncTermHelper {
    param(
        [Parameter(Mandatory = $true)][string[]]$@VAR_Arguments@
    )
    if ([string]::IsNullOrEmpty($env:@HELPER_ENV@)) {
        throw '@HELPER_ENV@ is not set'
    }
    $@VAR_startInfo@ = [Diagnostics.ProcessStartInfo]::new($env:@HELPER_ENV@)
    $@VAR_startInfo@.UseShellExecute = $false
    $@VAR_startInfo@.RedirectStandardOutput = $true
    $@VAR_startInfo@.RedirectStandardError = $true
    $@VAR_startInfo@.StandardOutputEncoding = [Text.Encoding]::UTF8
    $@VAR_startInfo@.StandardErrorEncoding = [Text.Encoding]::UTF8
    foreach ($@VAR_argument@ in $@VAR_Arguments@) {
        $@VAR_startInfo@.ArgumentList.Add($@VAR_argument@)
    }
    $@VAR_startProcess@ = $null
    try {
        $@VAR_startProcess@ = [Diagnostics.Process]::Start($@VAR_startInfo@)
        if ($null -eq $@VAR_startProcess@) {
            throw 'FuncTerm helper did not start a process'
        }
        $@VAR_stdoutRead@ = $@VAR_startProcess@.StandardOutput.ReadToEndAsync()
        $@VAR_stderrRead@ = $@VAR_startProcess@.StandardError.ReadToEndAsync()
        $@VAR_startProcess@.WaitForExit()
        $@VAR_helperStdout@ = $@VAR_stdoutRead@.GetAwaiter().GetResult()
        $@VAR_helperStderr@ = $@VAR_stderrRead@.GetAwaiter().GetResult()
        if ($@VAR_startProcess@.ExitCode -ne 0) {
            throw ('FuncTerm helper {0} failed with exit code {1}: {2}' -f $@VAR_Arguments@[0], $@VAR_startProcess@.ExitCode, $@VAR_helperStderr@)
        }
        if ($@VAR_helperStderr@.Length -gt 0) {
            [Console]::Error.Write($@VAR_helperStderr@)
        }
        return $@VAR_helperStdout@
    }
    finally {
        if ($null -ne $@VAR_startProcess@) {
            $@VAR_startProcess@.Dispose()
        }
    }
}
function Publish-FuncTermStart {
    param(
        [Parameter(Mandatory = $true)][string]$@VAR_CommandId@,
        [Parameter(Mandatory = $true)][string]$@VAR_Directory@
    )
    $null = Invoke-FuncTermHelper -@VAR_Arguments@ @(
        'internal-write-start', '--command-id', $@VAR_CommandId@, '--directory', $@VAR_Directory@
    )
}" ;
pub(super) const POWERSHELL_SHIMS: &str = "function Set-FuncTermShimPath {
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
";
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
