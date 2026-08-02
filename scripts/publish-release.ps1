# publish-release.ps1 — 构建 + 签名 + 发布到 Gitea Release（自动更新源）
#
# 注意: 必须用 PowerShell 7+ 运行（PS 5.1 的原生命命令引号缺陷会导致上传失败）
#   pwsh -ExecutionPolicy Bypass -File scripts/publish-release.ps1
#
# 流程:
#   1. 读 GITEA_TOKEN（环境变量，缺失时回退用户环境变量）
#   2. 读签名私钥（TAURI_SIGNING_PRIVATE_KEY_PATH，默认 ~/.tauri/ms_manage.key）
#   3. npm run tauri:build（默认 bump minor 版本）→ 带私钥环境变量,产物自动签名
#   4. 打便携版 zip（build-zip.ps1）
#   5. 创建/更新 Gitea Release vX.Y.Z，上传:
#      - setup.exe + .sig 签名
#      - update.json（自动更新清单,客户端 endpoint 拉的就是它）
#      - 便携版 zip
#
# 用法:
#   powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1        # bump + 构建 + 发布
#   powershell -ExecutionPolicy Bypass -File scripts/publish-release.ps1 -NoBump  # 不 bump,用当前版本重发

param(
  [switch]$NoBump,
  [switch]$SkipBuild
)

$ErrorActionPreference = 'Stop'
$root = Resolve-Path (Join-Path $PSScriptRoot '..')
# 注意: 发布到公开的 ms-manage-releases 仓库（私有仓库匿名访问会 404,客户端拉不到更新）
$api = 'http://www.73x.space:9100/api/v1/repos/Liuqingshan/ms-manage-releases'
$base = 'http://www.73x.space:9100/Liuqingshan/ms-manage-releases/releases/download'

# ---------- 1. token ----------
$token = $env:GITEA_TOKEN
if (-not $token) { $token = [Environment]::GetEnvironmentVariable('GITEA_TOKEN', 'User') }
if (-not $token) {
  Write-Error 'GITEA_TOKEN not found. Run: setx GITEA_TOKEN "<your-gitea-token>"'
  exit 1
}
$headers = @{ Authorization = "token $token" }

# ---------- 2. 签名私钥 ----------
# 注意: tauri-cli 2.11 的 updater 签名阶段只认字符串变量 TAURI_SIGNING_PRIVATE_KEY,
# 不认路径变量 TAURI_SIGNING_PRIVATE_KEY_PATH,这里读文件内容再注入字符串。
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
  & powershell -ExecutionPolicy Bypass -File (Join-Path $root 'scripts\build-zip.ps1')
  if ($LASTEXITCODE -ne 0) { Write-Error 'build-zip failed'; exit 1 }
}

# ---------- 5. 发布 ----------
$pkg = Get-Content (Join-Path $root 'package.json') -Raw | ConvertFrom-Json
$ver = $pkg.version
$tag = "v$ver"
Write-Host ''
Write-Host "Publishing $tag ..." -ForegroundColor Cyan

$exe = Join-Path $root "src-tauri\target\release\bundle\nsis\MS Manage_${ver}_x64-setup.exe"
$sig = "$exe.sig"
$zip = Join-Path $root "dist-zip\MS-Manage-v${ver}-win-x64.zip"
foreach ($f in @($exe, $sig)) {
  if (-not (Test-Path $f)) { Write-Error "Missing artifact: $f"; exit 1 }
}
if (-not (Test-Path $zip)) { Write-Warning "portable zip not found (skip): $zip" }

# 创建 release（不存在时）
$release = $null
try { $release = Invoke-RestMethod -Uri "$api/releases/tags/$tag" -Headers $headers } catch { $release = $null }
if (-not $release) {
  $notes = (git -C $root log -3 --pretty=format:'- %s') -join "`n"
  $body = @{ tag_name = $tag; name = $tag; body = "MS Manage v$ver`n`n$notes"; draft = $false; prerelease = $false } | ConvertTo-Json
  $release = Invoke-RestMethod -Uri "$api/releases" -Method Post -Headers $headers -ContentType 'application/json' -Body $body
  Write-Host "release created: $tag" -ForegroundColor Green
} else {
  Write-Host "release exists: $tag (attachments will be updated)" -ForegroundColor Yellow
}
$releaseId = $release.id

# 上传/覆盖附件（用 curl.exe：multipart 上传）
# 注意: 本脚本必须用 pwsh (PowerShell 7+) 运行 —— PS 5.1 对含空格参数的
# 原生命令引号重建有缺陷,会破坏 URL,导致 curl 报 URL malformed。
function Upload-Asset([int]$id, [string]$path) {
  $name = Split-Path $path -Leaf
  $existing = @($release.assets | Where-Object { $_.name -eq $name })
  foreach ($a in $existing) {
    Invoke-RestMethod -Uri "$api/releases/$id/assets/$($a.id)" -Method Delete -Headers $headers | Out-Null
    Write-Host "removed old: $name (asset $($a.id))" -ForegroundColor Yellow
  }
  curl.exe -s -H "Authorization: token $token" -F "attachment=@$path" "$api/releases/$id/assets?name=$([uri]::EscapeDataString($name))" | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "upload failed: $name" }
  Write-Host "uploaded: $name" -ForegroundColor Green
}

# update.json（自动更新清单）: 客户端 endpoint 指向 .../releases/download/v{version}/update.json
$sigText = (Get-Content $sig -Raw).Trim()
$exeName = Split-Path $exe -Leaf
$updateJson = [ordered]@{
  version   = $ver
  notes     = "MS Manage v$ver"
  pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  platforms = [ordered]@{
    'windows-x86_64' = [ordered]@{
      signature = $sigText
      url       = "$base/$tag/$([uri]::EscapeDataString($exeName))"
    }
  }
}
$updateJsonPath = Join-Path $env:TEMP 'update.json'
[System.IO.File]::WriteAllText($updateJsonPath, ($updateJson | ConvertTo-Json -Depth 5), [System.Text.UTF8Encoding]::new($false))

# 刷新 release.assets（上传 update.json 之前查询）
$release = Invoke-RestMethod -Uri "$api/releases/$releaseId" -Headers $headers
Upload-Asset $releaseId $exe
Upload-Asset $releaseId $sig
Upload-Asset $releaseId $updateJsonPath
if (Test-Path $zip) { Upload-Asset $releaseId $zip }

Write-Host ''
Write-Host "Done: $tag" -ForegroundColor Green
Write-Host "Update endpoint: $base/$tag/update.json"
Write-Host 'Verify in browser: http://www.73x.space:9100/Liuqingshan/ms-manage-releases/releases'
