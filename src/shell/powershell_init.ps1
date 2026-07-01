function Invoke-McpPtyCommand {
    param(
        [Parameter(Mandatory = $true)][string]$CommandId,
        [Parameter(Mandatory = $true)][string]$Payload,
        [Parameter(Mandatory = $true)][string]$Directory,
        [Parameter(Mandatory = $true)][string]$WorkingDirectory
    )
    New-Item -ItemType Directory -Force -Path $Directory | Out-Null
    $stdoutFile = Join-Path $Directory 'stdout.txt'
    $stderrFile = Join-Path $Directory 'stderr.txt'
    $doneFile = Join-Path $Directory 'done.json'
    $doneTempFile = Join-Path $Directory 'done.json.tmp'
    Set-Content -LiteralPath $stdoutFile -Value '' -NoNewline -Encoding utf8
    Set-Content -LiteralPath $stderrFile -Value '' -NoNewline -Encoding utf8
    try {
        Set-Location -LiteralPath $WorkingDirectory
        $global:LASTEXITCODE = $null
        $script = [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($Payload))
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
    $done = @{
        command_id   = $CommandId
        exit_code    = $exitCode
        cwd          = (Get-Location).Path
        completed_at = (Get-Date).ToUniversalTime().ToString('o')
    } | ConvertTo-Json -Compress
    Set-Content -LiteralPath $doneTempFile -Value $done -Encoding utf8
    Move-Item -LiteralPath $doneTempFile -Destination $doneFile -Force
}
