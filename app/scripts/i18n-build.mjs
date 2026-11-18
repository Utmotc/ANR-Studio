// 把译文表（按 _i18n_raw.json 的 strings 顺序，一行一条）编译成字典 JSON。
// 用法：node scripts/i18n-build.mjs
import fs from 'node:fs';

const read = (p) => fs.readFileSync(new URL(p, import.meta.url), 'utf8');
const raw = JSON.parse(read('./_i18n_raw.json'));
const keys = raw.strings;

// 源码里 JS 字符串用 \\ 表示一个反斜杠，而 DOM 渲染出来是单个反斜杠，统一成后者
const norm = (s) => s.replace(/\\\\/g, '\\');

function build(file, out) {
  let lines = read(file).split(/\r?\n/);
  while (lines.length && lines[lines.length - 1].trim() === '') lines.pop();
  if (lines.length !== keys.length) {
    console.error(`${file}: 译文 ${lines.length} 行，文案 ${keys.length} 条，数量不符`);
    process.exit(1);
  }
  const dict = {};
  keys.forEach((k, i) => {
    const key = norm(k);
    const val = norm(lines[i].trim());
    if (val && val !== key) dict[key] = val;
  });
  fs.writeFileSync(new URL(out, import.meta.url), JSON.stringify(dict, null, 2), 'utf8');
  console.log(`${out}: ${Object.keys(dict).length} 条`);
  return dict;
}

fs.mkdirSync(new URL('../src/i18n/', import.meta.url), { recursive: true });
const en = build('./i18n-en.txt', '../src/i18n/en.json');
const sample = keys.slice(0, 6);
for (const k of sample) console.log('  ', k.slice(0, 40), '=>', en[norm(k)]?.slice(0, 60));

// 渲染时才发现的内联片段 / 动态文案：_missing.json（CDP 采集） + i18n-extra.txt
const missingPath = new URL('./_missing.json', import.meta.url);
if (fs.existsSync(missingPath)) {
  const missing = JSON.parse(read('./_missing.json'));
  let extra = read('./i18n-extra.txt').split(/\r?\n/);
  while (extra.length && extra[extra.length - 1].trim() === '') extra.pop();
  if (extra.length !== missing.length) {
    console.error(`i18n-extra.txt: ${extra.length} 行，缺失片段 ${missing.length} 条，数量不符`);
    process.exit(1);
  }
  missing.forEach((k, i) => {
    const val = extra[i].trim();
    en[norm(k)] = val === '<EMPTY>' ? '' : norm(val);
  });
  fs.writeFileSync(new URL('../src/i18n/en.json', import.meta.url), JSON.stringify(en, null, 2), 'utf8');
  console.log(`合并渲染片段后共 ${Object.keys(en).length} 条`);
}
