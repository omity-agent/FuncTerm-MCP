[CmdletBinding()]
param(
    [Alias('Host')]
    [string] $RemoteHost = 'mac',
    [string] $RemoteDir,
    [string] $TestCommand = 'cargo test',
    [string] $Msys2Root = $(if ($env:MSYS2_ROOT) { $env:MSYS2_ROOT } elseif (Test-Path 'F:\msys2') { 'F:\msys2' } else { 'C:\msys64' }),
    [string] $SshConfigPath = (Join-Path $HOME '.ssh\config'),
    [string] $KnownHostsPath = (Join-Path $HOME '.ssh\known_hosts')
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
function ConvertTo-MsysPath([string] $Path) {
    $full = (Resolve-Path -LiteralPath $Path).ProviderPath
    if ($full -notmatch '^([A-Za-z]):\\(.*)$') {
        throw "Cannot convert to MSYS2 path: $full"
    }

    return '/' + $Matches[1].ToLowerInvariant() + '/' + ($Matches[2] -replace '\\', '/')
}
function ConvertTo-ShLiteral([string] $Value) {
    return "'" + $Value.Replace("'", "'\''") + "'"
}
function Invoke-LoggedNative([string] $Exe, [string[]] $ArgsForExe, [string] $ErrorText) {
    $oldNativeErrorPreference = Get-Variable PSNativeCommandUseErrorActionPreference -ValueOnly -ErrorAction SilentlyContinue
    $hasNativeErrorPreference = $null -ne (Get-Variable PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue)
    if ($hasNativeErrorPreference) {
        $PSNativeCommandUseErrorActionPreference = $false
    }
    try {
        & $Exe @ArgsForExe
        if ($LASTEXITCODE -ne 0) {
            throw "${ErrorText}: exit code $LASTEXITCODE"
        }
    }
    finally {
        if ($hasNativeErrorPreference) {
            $PSNativeCommandUseErrorActionPreference = $oldNativeErrorPreference
        }
    }
}
function Invoke-SshScript([string] $ScriptText) {
    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $ssh
    $startInfo.UseShellExecute = $false
    $startInfo.RedirectStandardInput = $true
    foreach ($arg in ($sshArgs + @($RemoteHost, 'bash', '-s'))) {
        [void] $startInfo.ArgumentList.Add($arg)
    }
    $process = [Diagnostics.Process]::Start($startInfo)
    try {
        $process.StandardInput.NewLine = "`n"
        $process.StandardInput.Write(($ScriptText -replace "`r`n?", "`n").TrimEnd("`n") + "`n")
        $process.StandardInput.Close()
        $process.WaitForExit()
        return $process.ExitCode
    }
    finally {
        $process.Dispose()
    }
}
$repoRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).ProviderPath
$repoName = Split-Path -Leaf $repoRoot
if ([string]::IsNullOrWhiteSpace($RemoteDir)) {
    $safeUser = [Environment]::UserName -replace '[^A-Za-z0-9._-]', '_'
    $RemoteDir = "/tmp/$repoName-$safeUser"
}
$rsync = Join-Path $Msys2Root 'usr\bin\rsync.exe'
$ssh = Join-Path $Msys2Root 'usr\bin\ssh.exe'
if (-not (Test-Path -LiteralPath $rsync -PathType Leaf)) { throw "rsync not found: $rsync" }
if (-not (Test-Path -LiteralPath $ssh -PathType Leaf)) { throw "ssh not found: $ssh" }
$sshArgs = @()
if (Test-Path -LiteralPath $SshConfigPath -PathType Leaf) {
    $sshArgs += @('-F', (ConvertTo-MsysPath $SshConfigPath))
}
if (Test-Path -LiteralPath $KnownHostsPath -PathType Leaf) {
    $sshArgs += @('-o', "UserKnownHostsFile=$(ConvertTo-MsysPath $KnownHostsPath)")
}
foreach ($key in @('id_ed25519', 'id_rsa')) {
    $keyPath = Join-Path $HOME ".ssh\$key"
    if (Test-Path -LiteralPath $keyPath -PathType Leaf) {
        $sshArgs += @('-i', (ConvertTo-MsysPath $keyPath))
    }
}
$sshArgs += @(
    '-o', 'BatchMode=yes',
    '-o', 'StrictHostKeyChecking=accept-new',
    '-o', 'ServerAliveInterval=30',
    '-o', 'ServerAliveCountMax=4'
)
$remoteDirQ = ConvertTo-ShLiteral $RemoteDir
if ((Invoke-SshScript "mkdir -p $remoteDirQ") -ne 0) {
    throw 'failed to create remote dir'
}
$oldArgConvExcl = $env:MSYS2_ARG_CONV_EXCL
$env:MSYS2_ARG_CONV_EXCL = '*'
try {
    Invoke-LoggedNative -Exe $rsync -ArgsForExe @(
        '--archive',
        '--compress',
        '--delete',
        '--partial',
        '--delay-updates',
        '--itemize-changes',
        '--progress',
        '--stats',
        '--exclude=/target/',
        '--exclude=/.git/',
        '--exclude=/.DS_Store',
        '--rsh', "$(ConvertTo-MsysPath $ssh) $($sshArgs -join ' ')",
        "$(ConvertTo-MsysPath $repoRoot)/",
        "${RemoteHost}:$RemoteDir/"
    ) -ErrorText 'rsync failed'
}
finally {
    $env:MSYS2_ARG_CONV_EXCL = $oldArgConvExcl
}
$remoteTest = @"
cd $remoteDirQ
export PATH="`$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin:`$PATH"
[ -f "`$HOME/.cargo/env" ] && . "`$HOME/.cargo/env"
$TestCommand
"@
exit (Invoke-SshScript $remoteTest)
