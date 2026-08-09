#!/usr/bin/env node
/**
 * Release helper — 读 .sig → 生成 v{version}/update.json (Tauri 2 updater manifest)
 * + latest.json (兼容旧 endpoint)。
 *
 * Tauri 2.x 的 `tauri build` 自动生成 .exe + .sig, 但**不**生成 update.json —
 * 需要 CI/CD 步骤或这个脚本。
 *
 * docs/RELEASE.md 设计: 每个版本一个目录 (v1.7.0/update.json / v1.8.0/update.json),
 * 内容指向最新版本 (1.8.0)。1.7.0 拉 v1.7.0/update.json → 升级 → 1.8.0。
 * 1.8.0 拉 v1.8.0/update.json → 同版本 → latest。
 *
 * 用法:
 *   node scripts/release.mjs [version]    # default = 读 package.json
 *
 * 输出 (在 src-tauri/target/release/bundle/nsis/):
 *   - latest.json                            (兼容 endpoint /latest.json 风格)
 *   - v{version}/update.json                  (1.7.0 client 拉这个升级)
 *   - v{version_minor-1}/update.json          (1.6.0/1.7.0 等老版本拉这个)
 *
 * 后续可加更多版本 (例如 node scripts/release.mjs 1.9.0 会同时输出 v1.8.0/
 * v1.9.0/update.json, 因为 1.8.0 client 也要能升级到 1.9.0)。
 */
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const BUNDLE = resolve(ROOT, "src-tauri", "target", "release", "bundle", "nsis");

// 1. 读 version
const pkg = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8"));
const version = process.argv[2] || pkg.version;

// 2. 定位 .exe + .sig
const exeName = `MS Manage_${version}_x64-setup.exe`;
const exePath = resolve(BUNDLE, exeName);
const sigPath = `${exePath}.sig`;

if (!existsSync(exePath)) {
  console.error(`❌ missing exe: ${exePath}`);
  console.error(`   跑 npm run tauri:build:debug 或 tauri:build:nobump 先生成。`);
  process.exit(1);
}
if (!existsSync(sigPath)) {
  console.error(`❌ missing sig: ${sigPath}`);
  console.error(`   build 时没设 TAURI_SIGNING_PRIVATE_KEY (在 C:\\Users\\admin\\.tauri\\ms_manage.key)。`);
  process.exit(1);
}

// 3. 读 .sig → base64
const sigBytes = readFileSync(sigPath);
const signatureB64 = sigBytes.toString("base64");

// 4. 读 tauri.conf.json 拿 endpoint (含 {{current_version}} 占位符)
const tauriConf = JSON.parse(readFileSync(resolve(ROOT, "src-tauri", "tauri.conf.json"), "utf8"));
const endpointTemplate = (tauriConf.plugins?.updater?.endpoints ?? [])[0]
  ?? "https://REPLACE-ME/v{{current_version}}/update.json";
const pubDate = new Date().toISOString();

// 5. 生成 v{N}/update.json 的 url: 替换 {{current_version}} + 把末尾 update.json 换成 .exe 名
function buildVersionEndpoint(targetVersion) {
  // 把 .../v{{current_version}}/update.json 替换成 .../v{targetVersion}/encodeURIComponent(exe)
  return endpointTemplate
    .replace("{{current_version}}", targetVersion)
    .replace(/\/update\.json$/, `/${encodeURIComponent(exeName)}`);
}

// 6. 单个 manifest 工厂
function buildManifest() {
  return {
    version,
    pub_date: pubDate,
    notes: `Release via scripts/release.mjs (dev version ${version})`,
    platforms: {
      "windows-x86_64": {
        signature: signatureB64,
        url: buildVersionEndpoint(version),
      },
    },
  };
}

// 7. 生成文件列表 (RELEASE.md 设计: 每个版本目录都放 manifest, 内容指最新)
// 默认: latest.json + v1.7.0/update.json + v1.8.0/update.json
// 想发 1.7.x → 1.8.x: 客户端拉 v1.7.0/update.json, 内容 version=1.8.0 + url 指向 1.8.0 安装包
const targetVersions = new Set([version]);
// 1.8.0 → 自动加 1.7.0 (前一个 minor) 让 1.7.x 用户能升级
const [major, minor] = version.split(".").map(Number);
if (major > 0 || minor > 0) {
  const prevMinor = minor > 0 ? `${major}.${minor - 1}.0` : `${major - 1}.0.0`;
  targetVersions.add(prevMinor);
  // 1.8.0 也覆盖 1.6.x (前两个 minor) — 让 1.6.x 也能跳级
  if (minor >= 2) {
    targetVersions.add(`${major}.${minor - 2}.0`);
  }
}

const written = [];
for (const v of targetVersions) {
  const manifest = buildManifest();
  // url 改成指向 *当前* version (1.8.0) 的安装包, 即使 manifest 在 v{旧}/ 目录
  manifest.platforms["windows-x86_64"].url = buildVersionEndpoint(version);
  manifest.notes = `For v${v} client: upgrade to v${version}`;

  const versionDir = resolve(BUNDLE, `v${v}`);
  mkdirSync(versionDir, { recursive: true });
  const manifestPath = resolve(versionDir, "update.json");
  writeFileSync(manifestPath, JSON.stringify(manifest, null, 2) + "\n");
  written.push(manifestPath);
}

// latest.json (兼容旧 endpoint / latest.json 风格)
const latestManifest = buildManifest();
const latestPath = resolve(BUNDLE, "latest.json");
writeFileSync(latestPath, JSON.stringify(latestManifest, null, 2) + "\n");
written.push(latestPath);

console.log(`✅ wrote ${written.length} updater manifest(s):`);
for (const p of written) {
  console.log(`   - ${p.replace(BUNDLE + "\\", "")}`);
}
console.log(`\n   version: ${version} | pub_date: ${pubDate}`);
console.log(`   signature: ${signatureB64.length} bytes (base64)`);

// 8. 打印 scp / rsync 命令示例
const remoteDir = endpointTemplate
  .replace("{{current_version}}", version)
  .replace(/\/[^/]+$/, "");
console.log(`\n📦 传产物到 server (按你 server 选):`);
console.log(`   目标: ${remoteDir}`);
console.log(`\n   # scp:`);
console.log(`   scp '${exePath}' '${sigPath}' ${written.map(p => `'${p}'`).join(' ')} user@server:${remoteDir}/`);
console.log(`\n   # rsync (整目录同步):`);
console.log(`   rsync -av '${BUNDLE}/' user@server:${remoteDir}/  # 同步 .exe/.sig/各 v 版本 update.json`);