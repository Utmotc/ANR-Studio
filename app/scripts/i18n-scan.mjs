// 扫描前端源码里所有中文文案，生成翻译字典骨架。
// 用法：node scripts/i18n-scan.mjs
import fs from 'node:fs';

const root = new URL('../src/', import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, '$1');
const files = fs
  .globSync('**/*.{ts,tsx}', { cwd: root })
  .filter((f) => !f.includes('i18n/'))
  .sort();

const CJK = /[\u4e00-\u9fff]/;
const hits = new Map(); // 原文 -> 次数
const dynamic = new Set();

const patterns = [
  /'([^'\n]*[\u4e00-\u9fff][^'\n]*)'/g,
  /"([^"\n]*[\u4e00-\u9fff][^"\n]*)"/g,
  /`([^`\n]*[\u4e00-\u9fff][^`\n]*)`/g,
  />([^<>{}\n]*[\u4e00-\u9fff][^<>{}\n]*)</g,
];
// 明显是代码片段而不是界面文案的，丢掉
const looksLikeCode = (s) => /\b(const|let|return|function|import|export)\b|=>|\{|\}|\bmap\.|\bawait\b/.test(s);

for (const rel of files) {
  const text = fs.readFileSync(new URL('../src/' + rel, import.meta.url), 'utf8');
  for (const re of patterns) {
    re.lastIndex = 0;
    let m;
    while ((m = re.exec(text))) {
      const raw = m[1].trim();
      if (!raw || !CJK.test(raw) || looksLikeCode(raw)) continue;
      if (raw.includes('${')) {
        dynamic.add(raw);
        // 模板里的中文片段也收进字典，靠子串替换兜底
        for (const seg of raw.split(/\$\{[^}]*\}/)) {
          const s = seg.trim();
          if (s && CJK.test(s)) hits.set(s, (hits.get(s) ?? 0) + 1);
        }
        continue;
      }
      hits.set(raw, (hits.get(raw) ?? 0) + 1);
    }
  }
}

const out = {
  count: hits.size,
  dynamic: [...dynamic].sort(),
  strings: [...hits.keys()].sort((a, b) => b.length - a.length),
};
fs.writeFileSync(new URL('./_i18n_raw.json', import.meta.url), JSON.stringify(out, null, 2), 'utf8');
console.log('唯一文案:', out.count, '| 含插值模板:', out.dynamic.length);
console.log('最长的前 25 条:');
for (const s of out.strings.slice(0, 25)) console.log('  ', s.slice(0, 120));
console.log('含插值样本:');
for (const s of out.dynamic.slice(0, 15)) console.log('  ', s.slice(0, 120));
