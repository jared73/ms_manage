# publish-release.ps1 — 构建 + 签名 + 发布到 GitHub Release
#
# 注意: 必须用 PowerShell 7+ 运行（PS 5.1 的原生命令引号缺陷会导致上传失败）
#   pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1
#
# 自动更新机制:
#   客户端 endpoint: https://github.com/jared73/ms_manage/releases/download/v{ver}/update.json
#   (见 src-tauri/tauri.conf.json → plugins.updater.endpoints)
#
# 流程:
#   1. 读 GITHUB_TOKEN (环境变量，缺失时回退用户环境变量)
#   2. 读签名私钥 (TAURI_SIGNING_PRIVATE_KEY_PATH，默认 ~/.tauri/ms_manage.key)
#   3. npm run tauri:build (默认 bump minor 版本) → 带私钥环境变量,产物自动签名
#   4. 打便携版 zip (build-zip.ps1)
#   5. 创建/更新 GitHub Release vX.Y.Z,上传:
#      - setup.exe + .sig 签名
#      - update.json (自动更新清单,客户端拉的就是它)
#      - 便携版 zip + dist-portable/MS Manage.exe
#
# 用法:
#   pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1        # bump + 构建 + 发布
#   pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -NoBump  # 不 bump,用当前版本重发
#   pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -SkipBuild  # 跳过构建,仅重发附件

param(
  [switch]$NoBump,
  [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
$owner = 'jared73'
$repo = 'ms_manage'
$api = "https://api.github.com/repos/$owner/$repo"
$base = "https://github.com/$owner/$repo/releases/download"

# ---------- 1. token ----------
$token = $env:GITHUB_TOKEN
if (-not $token) { $token = [Environment]::GetEnvironmentVariable('GITHUB_TOKEN', 'User') }
if (-not $token) {
  Write-Error 'GITHUB_TOKEN not found. Run: setx GITHUB_TOKEN "<your-pat-with-repo-scope>"'
  exit 1
}
$headers = @{ Authorization = "token $token"; Accept = 'application/vnd.github+json' }

# ---------- 2. 签名私钥 ----------
$keyPath = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
if (-not $keyPath) { $keyPath = Join-Path $HOME '.tauri\ms_manage.key' }
if (-not (Test-Path $keyPath)) {
  Write-Error "Signing key not found: $keyPath (run: npm run tauri -- signer generate --ci)"
  exit 1
}
$keyPass = $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD
if (-not $keyPass) { $keyPass = [Environment]::GetEnvironmentVariable('TAURI_SIGNING_PRIVATE_KEY_PASSWORD', 'User') }
$env:TAURI_SIGNING_PRIVATE_KEY = [System.IO.File]::ReadAllText($keyPath).Trim()
if ($keyPass) { $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $keyPass }

# ---------- 3. 构建 ----------
Push-Location $root
try {
  if (-not $SkipBuild) {
    if ($NoBump) {
      npm run tauri:build:nobump
    } else {
      npm run tauri:build
    }
    if ($LASTEXITCODE -ne 0) { Write-Error 'tauri build failed'; exit 1 }
  }
} finally { Pop-Location }

# ---------- 4. 便携版 zip ----------
if (-not $SkipBuild) {
  & pwsh -ExecutionPolicy Bypass -File (Join-Path $root 'scripts\build-zip.ps1')
  if ($LASTEXITCODE -ne 0) { Write-Error 'build-zip failed'; exit 1 }
}

# ---------- 5. 发布 ----------
$pkg = Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json
$ver = $pkg.version
$tag = "v$ver"
Write-Host ''
Write-Host "Publishing $tag to GitHub ..." -ForegroundColor Cyan

$exe = Get-ChildItem -Path (Join-Path $root 'src-tauri\target\release\bundle\nsis') -Filter "MS Manage_${ver}_x64-setup.exe" | Select-Object -First 1
if (-not $exe) { Write-Error "Missing artifact: MS Manage_${ver}_x64-setup.exe"; exit 1 }
$sig = "$($exe.FullName).sig"
$zip = Join-Path $root "dist-zip\MS-Manage-v${ver}-win-x64.zip"
foreach ($f in @($exe.FullName, $sig)) {
  if (-not (Test-Path $f)) { Write-Error "Missing artifact: $f"; exit 1 }
}

# 创建 release (不存在时)
$release = $null
try { $release = Invoke-RestMethod -Uri "$api/releases/tags/$tag" -Headers $headers } catch { $release = $null }
if (-not $release) {
  $notes = (git -C $root log -3 --pretty=format:'- %s') -join "`n"
  $body = @{
    tag_name    = $tag
    name        = $tag
    body        = "MS Manage $tag`n`n$notes"
    draft       = $false
    prerelease  = $false
    target_commitish = (git -C $root rev-parse HEAD)
  } | ConvertTo-Json
  $release = Invoke-RestMethod -Uri "$api/releases" -Method Post -Headers $headers -ContentType 'application/json' -Body $body
  Write-Host "release created: $tag" -ForegroundColor Green
} else {
  Write-Host "release exists: $tag (attachments will be updated)" -ForegroundColor Yellow
}
$releaseId = $release.id

# 重新拉一次 release (含 assets 列表)
$release = Invoke-RestMethod -Uri "$api/releases/$releaseId" -Headers $headers

# 上传/覆盖附件
function Upload-Asset([int]$id, [string]$path) {
  $name = Split-Path $path -Leaf
  $existing = @($release.assets | Where-Object { $_.name -eq $name })
  foreach ($a in $existing) {
    Invoke-RestMethod -Uri "$api/releases/assets/$($a.id)" -Method Delete -Headers $headers | Out-Null
    Write-Host "removed old: $name (asset $($a.id))" -ForegroundColor Yellow
  }
  curl.exe -s -H "Authorization: token $token" -H 'Accept: application/vnd.github+json' `
    -F "attachment=@$path" `
    "$api/releases/$releaseId/assets?name=$([uri]::EscapeDataString($name))" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "upload failed: $name" }
  Write-Host "uploaded: $name" -ForegroundColor Green
}

# update.json (自动更新清单): 客户端 endpoint 指向 .../releases/download/v{version}/update.json
$sigText = (Get-Content $sig -Raw).Trim()
$updateJson = [ordered]@{
  version   = $ver
  notes     = "MS Manage $tag"
  pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  platforms = [ordered]@{
    'windows-x86_64' = [ordered]@{
      signature = $sigText
      url       = "$base/$tag/$([uri]::EscapeDataString($exe.Name))"
    }
  }
}
$updateJsonPath = Join-Path $env:TEMP 'update.json'
[System.IO.File]::WriteAllText($updateJsonPath, ($updateJson | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))

# 刷新 release.assets (上传 update.json 之前查询)
$release = Invoke-RestMethod -Uri "$api/releases/$releaseId" -Headers $headers
Upload-Asset $releaseId $exe.FullName
Upload-Asset $releaseId $sig
Upload-Asset $releaseId $updateJsonPath
if (Test-Path $zip) { Upload-Asset $releaseId $zip }

# 同步本地便携版目录 (dist-portable 是用户常取的绿色版目录)
Copy-Item (Join-Path $root "src-tauri\target\release\ms_manage.exe") (Join-Path $root "dist-portable\MS Manage.exe") -Force
Write-Host "portable synced: dist-portable\MS Manage.exe" -ForegroundColor Green

Write-Host ''
Write-Host "Done: $tag" -ForegroundColor Green
Write-Host "Update endpoint: $base/$tag/update.json"
Write-Host "Verify in browser: $base/tag/$tag"