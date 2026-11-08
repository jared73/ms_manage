param(
    [Parameter(Mandatory = $true)][string]$Sha,
    [int]$TimeoutMinutes = 0,
    [int]$IntervalSeconds = 20,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
if ($TimeoutMinutes -le 0) { $TimeoutMinutes = [int]$config.limits.max_ci_wait_minutes }
$owner = $config.repository.github.owner
$repo = $config.repository.github.repo

$deadline = (Get-Date).AddMinutes($TimeoutMinutes)
$lastSummary = ''

while ((Get-Date) -lt $deadline) {
    $response = Invoke-GitHubApi -Path "/repos/$owner/$repo/commits/$Sha/check-runs?per_page=100"
    $runs = @()
    if ($null -ne $response -and $null -ne $response.PSObject.Properties['check_runs']) {
        $runs = @($response.check_runs | ForEach-Object { $_ })
    }

    if ($runs.Count -gt 0) {
        $pending = @($runs | Where-Object { $_.status -ne 'completed' })
        $lastSummary = ($runs | ForEach-Object { "$($_.name)=$($_.status)/$($_.conclusion)" }) -join '; '

        if ($pending.Count -eq 0) {
            $failed = @($runs | Where-Object { $_.conclusion -notin @('success', 'neutral', 'skipped') })
            $result = [pscustomobject]@{
                ok      = ($failed.Count -eq 0)
                sha     = $Sha
                summary = $lastSummary
                failed  = @($failed | ForEach-Object { "$($_.name): $($_.conclusion) $($_.html_url)" })
            }
            if ($Json) { $result | ConvertTo-Json -Depth 5 } else {
                if ($result.ok) { Write-Output "CI PASSED for $Sha" } else { Write-Output "CI FAILED for $Sha`n$($result.failed -join "`n")" }
            }
            if ($result.ok) { exit 0 } else { exit 1 }
        }
    }

    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] waiting for CI on $Sha ... $lastSummary"
    Start-Sleep -Seconds $IntervalSeconds
}

Write-Error "Timed out after $TimeoutMinutes minutes waiting for CI on $Sha. Last state: $lastSummary"
exit 1
