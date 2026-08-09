#!/usr/bin/env node
/**
 * i18n key 一致性校验 —— 跑 `node scripts/check-i18n.mjs`。
 * 退出码：0 一致，1 不一致。
 *
 * 校验 src/i18n/en.ts 和 src/i18n/zh.ts 的顶层 key 集合是否一致。
 * 顶层 key 形式：`  keyName: {`（2 空格缩进 + 单词 + : + {）。
 * 嵌套 key 不校验（避免深 diff 噪音）；改 nested key 时两边手动同步。
 */
import fs from 'node:fs';
import path from 'node:path';
import url from 'node:url';

const __filename = url.fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const ROOT = path.resolve(__dirname, '..');

const EN = path.join(ROOT, 'src/i18n/en.ts');
const ZH = path.join(ROOT, 'src/i18n/zh.ts');

function extractKeys(filepath) {
  const text = fs.readFileSync(filepath, 'utf8');
  // 顶层 key: 2 空格缩进 + 单词 + : + 空白 + {
  const re = /^  (\w+):\s*\{/gm;
  return new Set([...text.matchAll(re)].map(m => m[1]));
}

const enKeys = extractKeys(EN);
const zhKeys = extractKeys(ZH);

const onlyEn = [...enKeys].filter(k => !zhKeys.has(k)).sort();
const onlyZh = [...zhKeys].filter(k => !enKeys.has(k)).sort();

let errors = 0;
if (onlyEn.length) {
  console.error(`✗ en.ts 有 ${onlyEn.length} 个顶层 key 在 zh.ts 里缺失:`);
  onlyEn.forEach(k => console.error(`    - ${k}`));
  errors += onlyEn.length;
}
if (onlyZh.length) {
  console.error(`✗ zh.ts 有 ${onlyZh.length} 个顶层 key 在 en.ts 里缺失:`);
  onlyZh.forEach(k => console.error(`    - ${k}`));
  errors += onlyZh.length;
}

if (!errors) {
  console.log(`✓ i18n key 一致 (en ∩ zh, 仅校验顶层 key 共 ${enKeys.size} 个)`);
  console.log(`    en.ts: ${enKeys.size} top-level keys`);
  console.log(`    zh.ts: ${zhKeys.size} top-level keys`);
}
process.exit(errors ? 1 : 0);