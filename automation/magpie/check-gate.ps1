param(
    [string]$RepoRoot = '',
    [switch]$Quick,
    [switch]$SkipInstall,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

if (-not $RepoRoot) {
    $RepoRoot = Get-RepoRootFromMagpieDir -MagpieDir $MagpieDir
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$config = Read-MagpieConfig -MagpieDir $MagpieDir
$stateDir = Get-MagpieStateDir -MagpieDir $MagpieDir
$logDir = Get-MagpieLogDir -MagpieDir $MagpieDir
$stamp = Get-MagpieStamp
$logPath = Join-Path $logDir "gate-$stamp.log"

$quickSteps = @('diff-check', 'typecheck', 'lint', 'i18n', 'unit-tests')
$steps = @($config.gate.steps)
if ($Quick) {
    $steps = @($steps | Where-Object { $quickSteps -contains $_.name })
}
if ($env:MAGPIE_GATE_QUICK -eq '1') {
    # NAS/Linux runner: Rust/Tauri Windows build is handled by GitHub Actions;
    # keep the local gate to frontend checks to avoid requiring a Windows toolchain.
    $steps = @($steps | Where-Object { $_.name -notlike 'cargo-*' })
}

Write-MagpieLog -Message "Gate start: $RepoRoot (quick=$([bool]$Quick))" -LogPath $logPath

if (-not $SkipInstall) {
    $vitestName = if ($IsWindows) { 'vitest.cmd' } else { 'vitest' }
    $npmReady = (Test-Path -LiteralPath (Join-Path $RepoRoot "node_modules/.bin/$vitestName")) -and
                (Test-Path -LiteralPath (Join-Path $RepoRoot 'node_modules/typescript/package.json'))
    if (-not $npmReady) {
        Write-MagpieLog -Message 'node_modules missing or incomplete; running npm ci (this may take a few minutes).' -LogPath $logPath
        Push-Location -LiteralPath $RepoRoot
        try {
            & npm ci 2>&1 | Tee-Object -FilePath $logPath -Append | Out-Host
            if ($LASTEXITCODE -ne 0) {
                throw "npm ci failed with exit code $LASTEXITCODE"
            }
        }
        finally {
            Pop-Location
        }
    }
}

$results = @()
$failed = 0

foreach ($step in $steps) {
    $cwd = Join-Path $RepoRoot $step.cwd
    Write-MagpieLog -Message "==> $($step.name): $($step.command)" -LogPath $logPath
    $started = Get-Date

    Push-Location -LiteralPath $cwd
    try {
        $output = Invoke-Expression $step.command 2>&1
        $exitCode = $LASTEXITCODE
    }
    finally {
        Pop-Location
    }

    $duration = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
    foreach ($line in @($output)) {
        Add-Content -LiteralPath $logPath -Value $line -Encoding utf8
    }

    $status = if ($exitCode -eq 0) { 'PASS' } else { 'FAIL' }
    Write-MagpieLog -Message "    [$status] $($step.name) (${duration}s)" -Level $(if ($exitCode -eq 0) { 'INFO' } else { 'ERROR' }) -LogPath $logPath

    $results += [pscustomobject]@{
        name       = $step.name
        command    = $step.command
        exitCode   = $exitCode
        durationSec = $duration
    }
    if ($exitCode -ne 0) { $failed++ }
}

$summary = [pscustomobject]@{
    ok        = ($failed -eq 0)
    generatedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    repo      = $RepoRoot
    quick     = [bool]$Quick
    log       = $logPath
    steps     = $results
}
$summary | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $stateDir "gate-$stamp.json") -Encoding utf8

if ($Json) {
    $summary | ConvertTo-Json -Depth 6
} else {
    Write-Host ''
    $results | Format-Table -AutoSize
    Write-Host "Gate log: $logPath"
}

if ($failed -gt 0) {
    Write-MagpieLog -Message "Gate FAILED ($failed step(s))." -Level 'ERROR' -LogPath $logPath
    exit 1
}

Write-MagpieLog -Message 'Gate PASSED.' -LogPath $logPath
exit 0
