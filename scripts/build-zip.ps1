# build-zip.ps1 — 把 release 产物打成 ZIP 绿色版（解压即用）
# 用法：powershell -ExecutionPolicy Bypass -File scripts/build-zip.ps1

$ErrorActionPreference = 'Stop'
$projectRoot = Resolve-Path (Join-Path $PSScriptRoot '..')
$releaseExe  = Join-Path $projectRoot 'src-tauri\target\release\ai-manage.exe'
$outDir      = Join-Path $projectRoot 'dist-zip'
$version     = '0.1.0'
$zipName     = "AI-Manage-v$version-win-x64.zip"
$zipPath     = Join-Path $outDir $zipName

if (-not (Test-Path $releaseExe)) {
  Write-Error ("Not found: $releaseExe -- run `npm run tauri:build` first.")
  exit 1
}

if (-not (Test-Path $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }

# Stage area
$stage = Join-Path $env:TEMP ("ai-manage-zip-" + [guid]::NewGuid().ToString('N').Substring(0,8))
New-Item -ItemType Directory -Path $stage | Out-Null

# Main binary
Copy-Item $releaseExe (Join-Path $stage 'ai-manage.exe')

# README
if (Test-Path (Join-Path $projectRoot 'README.md')) {
  Copy-Item (Join-Path $projectRoot 'README.md') $stage
}

# E2E test scripts
if (Test-Path (Join-Path $projectRoot 'e2e')) {
  Copy-Item -Recurse (Join-Path $projectRoot 'e2e') $stage
}

# Quick setup guide
$guide = @'
AI Manage v0.1.0 - Windows 绿色版

【1】解压到任意目录

【2】运行 GUI
  双击 ai-manage.exe

【3】配置 Claude Desktop
  编辑 %APPDATA%\Claude\claude_desktop_config.json，
  在 mcpServers 下添加：

    "ai-manage": {
      "command": "C:\\path\\to\\ai-manage.exe",
      "args": ["proxy", "stdio"]
    }

  或打开 AI Manage -> Claude Desktop 页 -> 点 "导出代理模式"

【4】stdio 代理的 CLI 用法
  ai-manage.exe proxy stdio    启动 stdio MCP 代理
  ai-manage.exe help           查看全部命令

【5】数据目录
  %LOCALAPPDATA%\ai-manage\
'@
$guide | Out-File -FilePath (Join-Path $stage 'QUICKSTART.txt') -Encoding UTF8

# Zip it
if (Test-Path $zipPath) { Remove-Item $zipPath -Force }
Add-Type -AssemblyName System.IO.Compression.FileSystem
[System.IO.Compression.ZipFile]::CreateFromDirectory($stage, $zipPath, [System.IO.Compression.CompressionLevel]::Optimal, $false)

Remove-Item -Recurse -Force $stage

$size = (Get-Item $zipPath).Length
Write-Host ''
Write-Host ('Generated: ' + $zipPath) -ForegroundColor Green
Write-Host ('Size: ' + [Math]::Round($size/1MB, 2).ToString() + ' MB')
