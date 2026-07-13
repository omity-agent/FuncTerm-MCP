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
