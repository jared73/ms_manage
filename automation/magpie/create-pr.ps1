param(
    [Parameter(Mandatory = $true)][string]$Head,
    [string]$Base = '',
    [Parameter(Mandatory = $true)][string]$Title,
    [string]$Body = '',
    [switch]$Draft,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
if (-not $Base) { $Base = $config.repository.base_branch }
$owner = $config.repository.github.owner
$repo = $config.repository.github.repo

$request = @{
    title = $Title
    head  = $Head
    base  = $Base
    body  = $Body
    draft = [bool]$Draft
}

try {
    $pr = Invoke-GitHubApi -Method POST -Path "/repos/$owner/$repo/pulls" -Body $request
}
catch {
    # A retry after a partial failure must not create a duplicate PR.
    $existingRaw = Invoke-GitHubApi -Path "/repos/$owner/$repo/pulls?state=open&head=$owner`:$Head"
    $existing = @($existingRaw | ForEach-Object { $_ })
    if ($existing.Count -eq 0) { throw }
    $pr = $existing[0]
}

$result = [pscustomobject]@{
    number = $pr.number
    url    = $pr.html_url
    state  = $pr.state
    head   = $pr.head.ref
    base   = $pr.base.ref
}

if ($Json) {
    $result | ConvertTo-Json -Depth 5
} else {
    Write-Output "PR #$($result.number): $($result.url)"
}
