param(
    [Parameter(Mandatory = $true)][ValidateSet('issues', 'community', 'nightly')][string]$Loop,
    [string]$RepoRoot = '',
    [string]$Model = '',
    [switch]$DryRun,
    [switch]$AllowDirty,
    [switch]$AllowAnyBranch
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
if (-not $RepoRoot) { $RepoRoot = Get-RepoRootFromMagpieDir -MagpieDir $MagpieDir }
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
if (-not $Model) { $Model = $config.agent.model }

$stateDir = Get-MagpieStateDir -MagpieDir $MagpieDir
$logDir = Get-MagpieLogDir -MagpieDir $MagpieDir
$stamp = Get-MagpieStamp
$logPath = Join-Path $logDir "loop-$Loop-$stamp.log"

$promptMap = @{
    issues    = '01-issues.md'
    community = '02-community.md'
    nightly   = '03-nightly.md'
}
$promptPath = Join-Path $MagpieDir "loops\$($promptMap[$Loop])"
if (-not (Test-Path -LiteralPath $promptPath)) {
    throw "Loop prompt not found: $promptPath"
}

$branch = (& git -C $RepoRoot rev-parse --abbrev-ref HEAD 2>$null).Trim()
if (-not $DryRun) {
    if (-not $AllowAnyBranch -and $branch -ne $config.repository.base_branch) {
        throw "Automation must run on '$($config.repository.base_branch)' (current: '$branch'). Use a dedicated checkout/worktree, or pass -AllowAnyBranch for a manual test."
    }

    if ($config.automation.require_clean_tree -and -not $AllowDirty) {
        Assert-CleanWorkingTree -RepoRoot $RepoRoot
    }
}

$fullPrompt = @"
# Magpie 自动维护上下文

- 时间：$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss zzz')
- 循环：$Loop
- 仓库：$RepoRoot
- 配置文件：$MagpieDir\config.json
- 状态目录：$stateDir
- 日志目录：$logDir
- 分支：$branch
- 自动合并：$($config.automation.auto_merge)
- 自动发版：$($config.automation.auto_release)（模式：$($config.automation.release_mode)）

请严格按下面这份循环提示执行。你处于无人值守模式：不要向用户提问；无法决定的事项写入状态目录下的 questions 文件并跳过。执行前先读 docs/agent/CODE-STANDARDS.md 和 docs/agent/LESSONS.md。

---

$((Get-Content -LiteralPath $promptPath -Raw -Encoding utf8))
"@

$promptFile = Join-Path $stateDir "prompt-$Loop-$stamp.md"
Set-Content -LiteralPath $promptFile -Value $fullPrompt -Encoding utf8

if ($DryRun) {
    Write-Output "=== DRY RUN: $Loop ==="
    Write-Output "RepoRoot: $RepoRoot"
    Write-Output "Model: $Model"
    Write-Output "Prompt: $promptPath"
    Write-Output ''
    Write-Output $fullPrompt
    exit 0
}

$lockPath = Join-Path $stateDir 'run.lock'
if (Test-Path -LiteralPath $lockPath) {
    $lockText = Get-Content -LiteralPath $lockPath -Raw
    if ($lockText -match 'pid=(\d+)') {
        $lockPid = [int]$matches[1]
        if (Get-Process -Id $lockPid -ErrorAction SilentlyContinue) {
            Write-MagpieLog -Message "Another Magpie loop is running (pid $lockPid); skipping this run." -Level 'WARN' -LogPath $logPath
            exit 0
        }
    }
    Remove-Item -LiteralPath $lockPath -Force
}

Set-Content -LiteralPath $lockPath -Value "pid=$PID`nloop=$Loop`nstarted=$((Get-Date).ToString('o'))" -Encoding utf8

try {
    $opencodeScript = (Get-Command $config.agent.cli -ErrorAction Stop).Source
    $pwsh = (Get-Command pwsh -ErrorAction Stop).Source
    $outLog = Join-Path $logDir "loop-$Loop-$stamp.out.log"
    $errLog = Join-Path $logDir "loop-$Loop-$stamp.err.log"

    $arguments = @(
        '-NoProfile',
        '-ExecutionPolicy', 'Bypass',
        '-File', $opencodeScript,
        'run',
        '--auto',
        '--format', 'default',
        '--agent', $config.agent.agent,
        '--model', $Model,
        '--title', "magpie/$Loop/$stamp",
        '--file', $promptFile,
        '"执行附件中的 Magpie 自动化维护循环。"'
    )

    Write-MagpieLog -Message "Starting opencode loop '$Loop' with model '$Model' (timeout $($config.agent.timeout_minutes)m)." -LogPath $logPath
    $env:MAGPIE_LOOP = $Loop
    $env:MAGPIE_STATE_DIR = $stateDir
    $process = Start-Process -FilePath $pwsh -ArgumentList $arguments -WorkingDirectory $RepoRoot -NoNewWindow -PassThru -RedirectStandardOutput $outLog -RedirectStandardError $errLog
    $timeoutMs = [int]$config.agent.timeout_minutes * 60 * 1000

    if (-not $process.WaitForExit($timeoutMs)) {
        Write-MagpieLog -Message "Loop timed out after $($config.agent.timeout_minutes) minutes; terminating." -Level 'ERROR' -LogPath $logPath
        & taskkill.exe /PID $process.Id /T /F 2>&1 | Out-Null
        exit 124
    }

    $exitCode = $process.ExitCode
    Write-MagpieLog -Message "Loop '$Loop' finished with exit code $exitCode." -Level $(if ($exitCode -eq 0) { 'INFO' } else { 'ERROR' }) -LogPath $logPath

    if (Test-Path -LiteralPath $errLog) {
        $errTail = Get-Content -LiteralPath $errLog -Tail 30 -ErrorAction SilentlyContinue
        if ($errTail) { Write-MagpieLog -Message "stderr tail: $($errTail -join ' | ')" -Level 'WARN' -LogPath $logPath }
    }

    exit $exitCode
}
finally {
    if (Test-Path -LiteralPath $lockPath) {
        Remove-Item -LiteralPath $lockPath -Force -ErrorAction SilentlyContinue
    }
}
