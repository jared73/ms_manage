/**
 * 每次 `npm run tauri:build` 前自动执行，将版本号 +0.1。
 *
 * 单一真相源：package.json 的 version 字段。
 * 同步到：src-tauri/Cargo.toml、src-tauri/tauri.conf.json。
 *
 * 用法：
 *   node scripts/bump-version.cjs          # bump minor（1.4.0 → 1.5.0）
 *   node scripts/bump-version.cjs major    # bump major（1.4.0 → 2.0.0）
 *   node scripts/bump-version.cjs --dry    # 只打印，不写入
 */
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const PKG_PATH = path.join(ROOT, "package.json");
const CARGO_PATH = path.join(ROOT, "src-tauri", "Cargo.toml");
const TAURI_CONF_PATH = path.join(ROOT, "src-tauri", "tauri.conf.json");

const args = process.argv.slice(2);
const dry = args.includes("--dry");
const level = args.find((a) => ["major", "minor"].includes(a)) || "minor";

// 1. 读取当前版本
const pkg = JSON.parse(fs.readFileSync(PKG_PATH, "utf-8"));
const current = pkg.version;
// 支持 x.y 或 x.y.z 两种格式
const match = current.match(/^(\d+)\.(\d+)(?:\.(\d+))?$/);
if (!match) {
  console.error(`Invalid version format: ${current}. Expected x.y or x.y.z`);
  process.exit(1);
}

// 2. 计算新版本
let major = parseInt(match[1]);
let minor = parseInt(match[2]);

if (level === "major") {
  major += 1;
  minor = 0;
} else {
  minor += 1;
}
const next = `${major}.${minor}.0`;

if (dry) {
  console.log(`[dry] Would bump ${level}: ${current} → ${next}`);
  process.exit(0);
}

// 3. 写 package.json
pkg.version = next;
fs.writeFileSync(PKG_PATH, JSON.stringify(pkg, null, 2) + "\n");
console.log(`package.json: ${current} → ${next}`);

// 4. 写 Cargo.toml
let cargo = fs.readFileSync(CARGO_PATH, "utf-8");
cargo = cargo.replace(/^version = "[\d.]+"$/m, `version = "${next}"`);
fs.writeFileSync(CARGO_PATH, cargo);
console.log(`Cargo.toml:   ${current} → ${next}`);

// 5. 写 tauri.conf.json
let tauriConf = fs.readFileSync(TAURI_CONF_PATH, "utf-8");
tauriConf = tauriConf.replace(/"version": "[\d.]+"/, `"version": "${next}"`);
fs.writeFileSync(TAURI_CONF_PATH, tauriConf);
console.log(`tauri.conf:   ${current} → ${next}`);

console.log(`\nBumped ${level}: ${current} → ${next}`);
