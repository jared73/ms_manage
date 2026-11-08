param(
    [Parameter(Mandatory = $true)][int]$Number,
    [ValidateSet('merge', 'squash', 'rebase')][string]$Method = 'squash',
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
$owner = $config.repository.github.owner
$repo = $config.repository.github.repo

$pr = Invoke-GitHubApi -Path "/repos/$owner/$repo/pulls/$Number"
if ($pr.state -ne 'open') {
    throw "PR #$Number is not open (state=$($pr.state))."
}
if ($pr.mergeable -eq $false) {
    throw "PR #$Number is not mergeable: $($pr.mergeable_state)"
}

$body = @{ merge_method = $Method }
$result = Invoke-GitHubApi -Method PUT -Path "/repos/$owner/$repo/pulls/$Number/merge" -Body $body
$out = [pscustomobject]@{
    number  = $Number
    merged  = $result.merged
    message = $result.message
    sha     = $result.sha
}

if ($Json) {
    $out | ConvertTo-Json -Depth 5
} else {
    Write-Output "PR #$Number merged=$($out.merged) sha=$($out.sha)"
}
if (-not $out.merged) { exit 1 }
