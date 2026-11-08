param(
    [switch]$Execute,
    [string]$Bump = '',
    [switch]$SkipGate,
    [switch]$SkipPush,
    [switch]$Json,
    [string]$RepoRoot = ''
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
if (-not $Bump) { $Bump = [string]$config.automation.release_bump }
if ($Bump -notin @('minor', 'major')) { throw "Unsupported bump level: $Bump" }
if (-not $RepoRoot) {
    $repoRoot = Get-RepoRootFromMagpieDir -MagpieDir $MagpieDir
} else {
    $repoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
}
$stateDir = Get-MagpieStateDir -MagpieDir $MagpieDir
$logDir = Get-MagpieLogDir -MagpieDir $MagpieDir
$stamp = Get-MagpieStamp
$logPath = Join-Path $logDir "release-$stamp.log"

function Invoke-ReleaseGit {
    param([string[]]$Arguments)
    $result = Invoke-MagpieGit -RepoRoot $repoRoot -Arguments $Arguments
    if ($result.ExitCode -ne 0) {
        throw "git $($Arguments -join ' ') failed:`n$($result.Output)"
    }
    return $result.Output
}

Write-MagpieLog -Message "Release check in $repoRoot (execute=$([bool]$Execute), bump=$Bump)" -LogPath $logPath

$fetchOk = $true
try {
    Invoke-ReleaseGit @('fetch', 'github', 'main', '--tags', '--prune') | Out-Null
    Invoke-ReleaseGit @('fetch', 'origin', 'main', '--tags', '--prune') | Out-Null
}
catch {
    $fetchOk = $false
    if ($Execute) { throw }
    Write-MagpieLog -Message "Remote fetch failed; continuing with local refs for dry-run: $($_.Exception.Message)" -Level 'WARN' -LogPath $logPath
}

if ($fetchOk) {
    Invoke-ReleaseGit @('checkout', $config.repository.base_branch) | Out-Null
    Invoke-ReleaseGit @('pull', '--ff-only', 'github', $config.repository.base_branch) | Out-Null
} else {
    $currentBranch = (& git -C $repoRoot rev-parse --abbrev-ref HEAD 2>$null).Trim()
    if ($currentBranch -ne $config.repository.base_branch) {
        throw "Cannot verify release from branch '$currentBranch' without a successful fetch."
    }
}
Assert-CleanWorkingTree -RepoRoot $repoRoot

$latestTag = ''
$tagOutput = & git -C $repoRoot describe --tags --abbrev=0 2>$null
if ($LASTEXITCODE -eq 0) { $latestTag = "$tagOutput".Trim() }
$range = if ($latestTag) { "$latestTag..$($config.repository.base_branch)" } else { $config.repository.base_branch }
$commitLines = @(& git -C $repoRoot log $range --pretty=format:'%h %s' 2>$null)
$commitTypes = @($config.release.commit_types)
$typePattern = ($commitTypes | ForEach-Object { [regex]::Escape($_) }) -join '|'
$commits = @($commitLines | Where-Object {
    $_ -and $_ -match "^[0-9a-f]+ ($typePattern)(\([^)]+\))?!?:"
})

if ($commits.Count -eq 0) {
    $message = "No unreleased commits since $latestTag. Nothing to release."
    Write-MagpieLog -Message $message -LogPath $logPath
    if ($Json) { [pscustomobject]@{ ok = $true; released = $false; message = $message } | ConvertTo-Json -Depth 4 }
    else { Write-Output $message }
    exit 0
}

$pkg = Get-Content -LiteralPath (Join-Path $repoRoot 'package.json') -Raw -Encoding utf8 | ConvertFrom-Json
$current = "$($pkg.version)"
$parts = $current -split '\.'
if ($parts.Count -lt 2) { throw "Unsupported version format: $current" }
$major = [int]$parts[0]
$minor = [int]$parts[1]
if ($Bump -eq 'major') { $next = "$($major + 1).0.0" } else { $next = "$major.$($minor + 1).0" }
$tag = "$($config.release.tag_prefix)$next"

$existingTag = & git -C $repoRoot rev-parse --verify --quiet "refs/tags/$tag" 2>$null
if ($LASTEXITCODE -eq 0) {
    throw "Tag $tag already exists; refusing to overwrite."
}

Write-MagpieLog -Message "Current=$current Next=$next Tag=$tag Commits=$($commits.Count)" -LogPath $logPath
foreach ($c in $commits) { Write-MagpieLog -Message "  $c" -LogPath $logPath }

if (-not $Execute) {
    $plan = [pscustomobject]@{
        ok          = $true
        execute     = $false
        current     = $current
        next        = $next
        tag         = $tag
        commitCount = $commits.Count
        commits     = $commits
        message     = 'Dry run only. Re-run with -Execute to bump, tag, and push.'
    }
    if ($Json) { $plan | ConvertTo-Json -Depth 6 } else {
        Write-Output "DRY RUN: would release $current -> $next ($tag) with $($commits.Count) commit(s)."
        $commits | ForEach-Object { Write-Output "  $_" }
        Write-Output 'Add -Execute to perform the release.'
    }
    exit 0
}

if (-not $SkipGate) {
    Write-MagpieLog -Message 'Running full gate before release...' -LogPath $logPath
    & (Join-Path $MagpieDir 'check-gate.ps1') -RepoRoot $repoRoot
    if ($LASTEXITCODE -ne 0) {
        throw 'Release gate failed; refusing to bump or tag.'
    }
}

Write-MagpieLog -Message 'Bumping version files...' -LogPath $logPath
Push-Location -LiteralPath $repoRoot
try {
    & node 'scripts/bump-version.cjs' $Bump 2>&1 | Tee-Object -FilePath $logPath -Append | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "bump-version failed with exit code $LASTEXITCODE" }
}
finally {
    Pop-Location
}

$versionFiles = @($config.release.version_files)
Invoke-ReleaseGit (@('add', '--') + $versionFiles) | Out-Null
Invoke-ReleaseGit @('commit', '-m', "chore(release): bump $current -> $next") | Out-Null
Invoke-ReleaseGit @('tag', $tag) | Out-Null

if (-not $SkipPush) {
    Invoke-ReleaseGit @('push', $config.release.github_remote, $config.repository.base_branch) | Out-Null
    Invoke-ReleaseGit @('push', $config.release.github_remote, $tag) | Out-Null
    Invoke-ReleaseGit @('push', $config.release.origin_remote, $config.repository.base_branch) | Out-Null
    Invoke-ReleaseGit @('push', $config.release.origin_remote, $tag) | Out-Null
}

$url = "https://github.com/$($config.repository.github.owner)/$($config.repository.github.repo)/releases/tag/$tag"
$result = [pscustomobject]@{
    ok          = $true
    execute     = $true
    fromVersion = $current
    toVersion   = $next
    tag         = $tag
    url         = $url
    pushed      = (-not $SkipPush)
}
if ($Json) { $result | ConvertTo-Json -Depth 5 } else {
    Write-Output "Released $tag (triggered $($config.release.workflow))."
    Write-Output $url
}
