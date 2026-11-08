param(
    [ValidateSet('all', 'gitea', 'github')][string]$Source = 'all',
    [int]$Limit = 50,
    [switch]$Json
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
$repoRoot = Get-RepoRootFromMagpieDir -MagpieDir $MagpieDir

function Convert-BodyPreview {
    param([string]$Body, [int]$MaxLength = 8000)

    if (-not $Body) { return '' }
    if ($Body.Length -le $MaxLength) { return $Body }
    return $Body.Substring(0, $MaxLength) + "`n...[truncated]"
}

function Get-GiteaFeedback {
    param([object]$SourceConfig, [int]$MaxItems)

    $token = Get-GiteaToken -TokenEnv $SourceConfig.token_env
    $base = $config.repository.gitea.base_url.TrimEnd('/')
    $owner = $config.repository.gitea.owner
    $repo = $config.repository.gitea.repo
    $headers = @{
        Authorization = "token $token"
        Accept        = 'application/json'
    }

    $items = @()

    $issueUri = "$base/api/v1/repos/$owner/$repo/issues?state=open&type=issues&limit=$MaxItems"
    $issuesRaw = Invoke-RestMethod -Uri $issueUri -Headers $headers -TimeoutSec 30
    $issues = @($issuesRaw | ForEach-Object { $_ })
    foreach ($issue in $issues) {
        if ($null -eq $issue) { continue }
        $items += [pscustomobject]@{
            source          = 'gitea'
            kind            = 'issue'
            number          = $issue.number
            title           = $issue.title
            state           = $issue.state
            author          = $issue.user.login
            url             = $issue.html_url
            labels          = @($issue.labels | ForEach-Object { $_.name })
            isPullRequest   = $false
            createdAt       = $issue.created_at
            updatedAt       = $issue.updated_at
            body            = Convert-BodyPreview -Body $issue.body
        }
    }

    $pullUri = "$base/api/v1/repos/$owner/$repo/pulls?state=open&limit=$MaxItems"
    $pullsRaw = Invoke-RestMethod -Uri $pullUri -Headers $headers -TimeoutSec 30
    $pulls = @($pullsRaw | ForEach-Object { $_ })
    foreach ($pull in $pulls) {
        if ($null -eq $pull) { continue }
        $items += [pscustomobject]@{
            source          = 'gitea'
            kind            = 'pull'
            number          = $pull.number
            title           = $pull.title
            state           = $pull.state
            author          = $pull.user.login
            url             = $pull.html_url
            labels          = @($pull.labels | ForEach-Object { $_.name })
            isPullRequest   = $true
            createdAt       = $pull.created_at
            updatedAt       = $pull.updated_at
            body            = Convert-BodyPreview -Body $pull.body
        }
    }

    return $items
}

function Get-GitHubFeedback {
    param([object]$SourceConfig, [int]$MaxItems)

    $owner = $config.repository.github.owner
    $repo = $config.repository.github.repo
    $uri = "https://api.github.com/repos/$owner/$repo/issues?state=open&per_page=$MaxItems"
    try {
        $token = Get-GitHubToken
        $headers = @{ Authorization = "token $token"; Accept = 'application/vnd.github+json'; 'User-Agent' = 'ms-manage-magpie' }
    } catch {
        # Public repos can be read without a token, but the rate limit is low.
        $headers = @{ Accept = 'application/vnd.github+json'; 'User-Agent' = 'ms-manage-magpie' }
    }

    $issues = @()
    $issuesRaw = Invoke-RestMethod -Uri $uri -Headers $headers -TimeoutSec 30
    $issues = @($issuesRaw | ForEach-Object { $_ })
    $items = @()
    foreach ($issue in $issues) {
        if ($null -eq $issue) { continue }
        $kind = 'issue'
        if ($null -ne $issue.PSObject.Properties['pull_request']) { $kind = 'pull' }
        $items += [pscustomobject]@{
            source          = 'github'
            kind            = $kind
            number          = $issue.number
            title           = $issue.title
            state           = $issue.state
            author          = $issue.user.login
            url             = $issue.html_url
            labels          = @($issue.labels | ForEach-Object { $_.name })
            isPullRequest   = ($kind -eq 'pull')
            createdAt       = $issue.created_at
            updatedAt       = $issue.updated_at
            body            = Convert-BodyPreview -Body $issue.body
        }
    }

    return $items
}

$allItems = @()
foreach ($feedbackSource in @($config.feedback.sources)) {
    if (-not $feedbackSource.enabled) { continue }
    if ($Source -ne 'all' -and $feedbackSource.kind -ne $Source) { continue }

    try {
        switch ($feedbackSource.kind) {
            'gitea' { $allItems += Get-GiteaFeedback -SourceConfig $feedbackSource -MaxItems $Limit }
            'github' { $allItems += Get-GitHubFeedback -SourceConfig $feedbackSource -MaxItems $Limit }
            default { Write-Warning "Unknown feedback source kind: $($feedbackSource.kind)" }
        }
    }
    catch {
        $where = $_.InvocationInfo.PositionMessage
        Write-Warning "Failed to read $($feedbackSource.name): $($_.Exception.Message) $where"
    }
}

$allItems = @($allItems | Sort-Object -Property updatedAt -Descending)
$result = [pscustomobject]@{
    generatedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
    repo        = $repoRoot
    count       = $allItems.Count
    items       = $allItems
}

if ($Json) {
    $result | ConvertTo-Json -Depth 10
} else {
    if ($allItems.Count -eq 0) {
        Write-Output 'No open issues or pull requests.'
    } else {
        $allItems | Select-Object source, kind, number, state, updatedAt, title | Format-Table -AutoSize
    }
}
