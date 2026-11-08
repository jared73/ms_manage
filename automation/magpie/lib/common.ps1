# Shared helpers for the Magpie maintenance loops.
# Dot-source from automation/magpie/*.ps1:  . "$PSScriptRoot\lib\common.ps1"

Set-StrictMode -Version Latest

function Read-MagpieConfig {
    param(
        [Parameter(Mandatory = $true)][string]$MagpieDir
    )

    $path = Join-Path $MagpieDir 'config.json'
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Magpie config not found: $path"
    }
    return (Get-Content -LiteralPath $path -Raw -Encoding utf8 | ConvertFrom-Json)
}

function Get-MagpieStateDir {
    param(
        [Parameter(Mandatory = $true)][string]$MagpieDir
    )

    $dir = Join-Path $MagpieDir 'state'
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    return $dir
}

function Get-MagpieLogDir {
    param(
        [Parameter(Mandatory = $true)][string]$MagpieDir
    )

    $dir = Join-Path $MagpieDir 'logs'
    if (-not (Test-Path -LiteralPath $dir)) {
        New-Item -ItemType Directory -Path $dir -Force | Out-Null
    }
    return $dir
}

function Get-MagpieStamp {
    return (Get-Date -Format 'yyyyMMdd-HHmmss')
}

function Write-MagpieLog {
    param(
        [Parameter(Mandatory = $true)][string]$Message,
        [ValidateSet('INFO', 'WARN', 'ERROR')][string]$Level = 'INFO',
        [string]$LogPath = ''
    )

    $line = "[{0}] [{1}] {2}" -f (Get-Date -Format 'yyyy-MM-dd HH:mm:ss'), $Level, $Message
    Write-Host $line
    if ($LogPath) {
        Add-Content -LiteralPath $LogPath -Value $line -Encoding utf8
    }
}

function Get-EnvOrUserValue {
    param([Parameter(Mandatory = $true)][string]$Name)

    $value = [Environment]::GetEnvironmentVariable($Name, 'Process')
    if (-not $value) {
        $value = [Environment]::GetEnvironmentVariable($Name, 'User')
    }
    return $value
}

function Get-GitHubToken {
    $token = Get-EnvOrUserValue -Name 'GITHUB_TOKEN'
    if ($token) { return $token }

    # Fall back to Git Credential Manager so the loop works without duplicating
    # a PAT into a new environment variable.
    $oldPrompt = $env:GIT_TERMINAL_PROMPT
    $oldGcm = $env:GCM_INTERACTIVE
    $env:GIT_TERMINAL_PROMPT = '0'
    $env:GCM_INTERACTIVE = 'Never'
    try {
        $lines = "protocol=https`nhost=github.com`n`n" | git credential fill 2>$null
        if ($LASTEXITCODE -eq 0) {
            foreach ($line in $lines) {
                if ($line -match '^password=(.*)$') {
                    return $matches[1]
                }
            }
        }
    }
    finally {
        $env:GIT_TERMINAL_PROMPT = $oldPrompt
        $env:GCM_INTERACTIVE = $oldGcm
    }

    throw 'GitHub token not found. Set GITHUB_TOKEN or store a credential for https://github.com via Git Credential Manager.'
}

function Get-GiteaToken {
    param([Parameter(Mandatory = $true)][string]$TokenEnv)

    $token = Get-EnvOrUserValue -Name $TokenEnv
    if (-not $token) {
        throw "Gitea token not found. Set $TokenEnv."
    }
    return $token
}

function Invoke-MagpieGit {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot,
        [Parameter(Mandatory = $true)][string[]]$Arguments
    )

    Push-Location -LiteralPath $RepoRoot
    try {
        $output = & git @Arguments 2>&1
        $code = $LASTEXITCODE
        return [pscustomobject]@{
            ExitCode = $code
            Output   = (($output | ForEach-Object { "$_" }) -join "`n")
        }
    }
    finally {
        Pop-Location
    }
}

function Invoke-GitHubApi {
    param(
        [ValidateSet('GET', 'POST', 'PUT', 'PATCH', 'DELETE')][string]$Method = 'GET',
        [Parameter(Mandatory = $true)][string]$Path,
        [object]$Body = $null
    )

    $token = Get-GitHubToken
    $headers = @{
        Authorization = "token $token"
        Accept        = 'application/vnd.github+json'
        'User-Agent'  = 'ms-manage-magpie'
    }

    $params = @{
        Uri        = "https://api.github.com$Path"
        Method     = $Method
        Headers    = $headers
        TimeoutSec = 30
    }
    if ($null -ne $Body) {
        $params.Body = ($Body | ConvertTo-Json -Depth 12)
        $params.ContentType = 'application/json'
    }

    return Invoke-RestMethod @params
}

function Assert-CleanWorkingTree {
    param(
        [Parameter(Mandatory = $true)][string]$RepoRoot
    )

    $status = & git -C $RepoRoot status --porcelain 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "git status failed: $status"
    }
    if ($status) {
        throw "Working tree is not clean. Refusing to let an automated loop touch user changes:`n$($status -join "`n")"
    }
}

function Get-RepoRootFromMagpieDir {
    param([Parameter(Mandatory = $true)][string]$MagpieDir)

    return (Resolve-Path (Join-Path $MagpieDir '..\..')).Path
}
