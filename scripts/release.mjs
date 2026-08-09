#!/usr/bin/env node
/**
 * Release helper — 读 .sig 文件 → 生成 `latest.json`（Tauri 2 updater manifest）。
 *
 * Tauri 2.x 的 `tauri build` 会自动生成 .exe + .sig，
 * 但**不**生成 `latest.json`（那是 CI/CD 步骤或这个脚本的活）。
 *
 * 用法:
 *   node scripts/release.mjs [version]    # default = 读 package.json 的 version
 *
 * 输出:
 *   - target/release/bundle/latest.json   (Tauri 期望的 updater manifest 位置)
 *   - 命令行打印 scp/curl 命令示例, 让你传产物到 server
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { execSync } from "node:child_process";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const BUNDLE = resolve(ROOT, "src-tauri", "target", "release", "bundle", "nsis");

// 1. 读 version
const pkg = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8"));
const version = process.argv[2] || pkg.version;

// 2. 定位 .exe + .sig (Tauri 2 命名: "MS Manage_<version>_x64-setup.exe")
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

// 4. 读 tauri.conf.json 拿 endpoint + target triple
const tauriConf = JSON.parse(readFileSync(resolve(ROOT, "src-tauri", "tauri.conf.json"), "utf8"));
const endpoint = (tauriConf.plugins?.updater?.endpoints ?? [])[0]
  ?? "https://REPLACE-ME/update.json";
const pubDate = new Date().toISOString();

// 5. 写 latest.json (Tauri 2 updater 格式)
const manifest = {
  version,
  pub_date: pubDate,
  notes: "Release via scripts/release.mjs",
  platforms: {
    "windows-x86_64": {
      signature: signatureB64,
      url: endpoint
        .replace("{{current_version}}", version)
        .replace(/\/update\.json$/, `/${encodeURIComponent(exeName)}`),
    },
  },
};

const outPath = resolve(BUNDLE, "latest.json");
writeFileSync(outPath, JSON.stringify(manifest, null, 2) + "\n");
console.log(`✅ wrote ${outPath}`);
console.log(`   version: ${version}`);
console.log(`   pub_date: ${pubDate}`);
console.log(`   signature: ${signatureB64.length} bytes (base64)`);
console.log(`   url: ${manifest.platforms["windows-x86_64"].url}`);

// 6. 打印 scp / curl 命令示例 (用户按自己 server 选传输方式)
const remoteDir = endpoint
  .replace("{{current_version}}", version)
  .replace(/\/[^/]+$/, "");
console.log(`\n📦 传产物到 server (按你 server 选):`);
console.log(`   目标目录: ${remoteDir}`);
console.log(`\n   # scp 示例:`);
console.log(`   scp '${exePath}'   '${sigPath}'   '${outPath}'  user@server:${remoteDir}/`);
console.log(`\n   # 或者 rsync:`);
console.log(`   rsync -av '${BUNDLE}/' user@server:${remoteDir}/  # 同步 3 个文件`);
console.log(`\n   # 上传后, ms_manage Settings → 检查更新 → 拉 v${version}/latest.json → 检测新版本`);