param(
    [string]$RepoRoot = '',
    [switch]$Register,
    [switch]$Remove
)

$ErrorActionPreference = 'Stop'
$MagpieDir = $PSScriptRoot
. (Join-Path $MagpieDir 'lib\common.ps1')

$config = Read-MagpieConfig -MagpieDir $MagpieDir
if (-not $RepoRoot) { $RepoRoot = Get-RepoRootFromMagpieDir -MagpieDir $MagpieDir }
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$runScript = Join-Path $MagpieDir 'run-loop.ps1'
$pwsh = (Get-Command pwsh -ErrorAction Stop).Source

$taskPrefix = 'MSManage-Magpie'
$tasks = @(
    [pscustomobject]@{
        Name        = "$taskPrefix-Issues"
        Loop        = 'issues'
        Description = 'MS Manage Magpie loop: issue and PR patrol'
        Trigger     = 'interval'
        Minutes     = [int]$config.schedule.issues_interval_minutes
    },
    [pscustomobject]@{
        Name        = "$taskPrefix-Community"
        Loop        = 'community'
        Description = 'MS Manage Magpie loop: community feedback patrol'
        Trigger     = 'interval'
        Minutes     = [int]$config.schedule.community_interval_minutes
    },
    [pscustomobject]@{
        Name        = "$taskPrefix-Nightly"
        Loop        = 'nightly'
        Description = 'MS Manage Magpie loop: nightly cleanup, lessons and release'
        Trigger     = 'daily'
        At          = [string]$config.schedule.nightly_time
    }
)

if ($Remove) {
    foreach ($task in $tasks) {
        $existing = Get-ScheduledTask -TaskName $task.Name -ErrorAction SilentlyContinue
        if ($existing) {
            Unregister-ScheduledTask -TaskName $task.Name -Confirm:$false
            Write-Output "Removed: $($task.Name)"
        } else {
            Write-Output "Not found: $($task.Name)"
        }
    }
    exit 0
}

if (-not $Register) {
    Write-Output 'The following Windows Scheduled Tasks would be registered:'
    Write-Output ''
    foreach ($task in $tasks) {
        $arg = "-NoProfile -ExecutionPolicy Bypass -File `"$runScript`" -Loop $($task.Loop) -RepoRoot `"$RepoRoot`""
        $when = if ($task.Trigger -eq 'interval') { "every $($task.Minutes) minutes" } else { "daily at $($task.At)" }
        Write-Output "  $($task.Name)"
        Write-Output "    when:   $when"
        Write-Output "    action: `"$pwsh`" $arg"
        Write-Output ''
    }
    Write-Output 'Run with -Register to create them, or -Remove to delete them.'
    exit 0
}

$settings = New-ScheduledTaskSettingsSet `
    -AllowStartIfOnBatteries `
    -DontStopIfGoingOnBatteries `
    -StartWhenAvailable `
    -MultipleInstances IgnoreNew `
    -ExecutionTimeLimit (New-TimeSpan -Hours 2)

foreach ($task in $tasks) {
    $arg = "-NoProfile -ExecutionPolicy Bypass -File `"$runScript`" -Loop $($task.Loop) -RepoRoot `"$RepoRoot`""
    $action = New-ScheduledTaskAction -Execute $pwsh -Argument $arg

    if ($task.Trigger -eq 'interval') {
        $trigger = New-ScheduledTaskTrigger `
            -Once -At (Get-Date).AddMinutes(5) `
            -RepetitionInterval (New-TimeSpan -Minutes $task.Minutes) `
            -RepetitionDuration (New-TimeSpan -Days 3650)
    } else {
        $trigger = New-ScheduledTaskTrigger -Daily -At $task.At
    }

    Register-ScheduledTask `
        -TaskName $task.Name `
        -Action $action `
        -Trigger $trigger `
        -Settings $settings `
        -Description $task.Description `
        -Force | Out-Null

    Write-Output "Registered: $($task.Name)"
}
