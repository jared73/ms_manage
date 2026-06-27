# build-zip.ps1 — 把 release 产物打成 ZIP 绿色版（解压即用）
# 用法：powershell -ExecutionPolicy Bypass -File scripts/build-zip.ps1

$ErrorActionPreference = 'Stop'
$projectRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$releaseExe  = Join-Path $projectRoot 'src-tauri\target\release\ms_manage.exe'
$outDir      = Join-Path $projectRoot 'dist-zip'
$version     = '0.1.0'
$zipName     = "MS-Manage-v$version-win-x64.zip"
$zipPath     = Join-Path $outDir $zipName

if (-not (Test-Path $releaseExe)) {
  Write-Error ("Not found: $releaseExe -- run `cargo build --release --manifest-path src-tauri/Cargo.toml` first.")
  exit 1
}

if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }

# Stage area
$stage = Join-Path $env:TEMP ("ms-manage-zip-" + [guid]::NewGuid().ToString('N').Substring(0,8))
New-Item -ItemType Directory -Path $stage | Out-Null

# Main binary
Copy-Item $releaseExe (Join-Path $stage 'ms_manage.exe')

# README
if (Test-Path (Join-Path $projectRoot 'README.md')) {
  Copy-Item (Join-Path $projectRoot 'README.md') $stage
}

# E2E test scripts
if (Test-Path (Join-Path $projectRoot 'e2e')) {
  Copy-Item -Recurse (Join-Path $projectRoot 'e2e') $stage
}

# Quick setup guide —— 从独立 UTF-8 模板文件 copy（避免 PowerShell here-string 丢中文字符）
$template = Join-Path $PSScriptRoot 'QUICKSTART.template.txt'
if (Test-Path $template) {
    Copy-Item $template (Join-Path $stage 'QUICKSTART.txt')
} else {
    # 兜底：如果模板文件丢了，写个最小的
    [System.IO.File]::WriteAllText(
        (Join-Path $stage 'QUICKSTART.txt'),
        "MS Manage v0.1.0`nRun ms_manage.exe to start.`n",
        [System.Text.UTF8Encoding]::new($false)
    )
}

# Zip it
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($stage, $zipPath, [System.IO.Compression.CompressionLevel]::Optimal, $false)

Remove-Item -Recurse -Force $stage

$size = (Get-Item $zipPath).Length
Write-Host ''
Write-Host ('Generated: ' + $zipPath) -ForegroundColor Green
Write-Host ('Size: ' + [Math]::Round($size/1MB, 2).ToString() + ' MB')