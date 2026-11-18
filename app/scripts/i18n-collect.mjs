// 通过 CDP 遍历界面各状态，收集真正渲染出来的中文文本片段。
// 前置：用 --remote-debugging-port 启动 headless chrome 打开 dist/index.html
// 用法：CDP_PORT=9336 node scripts/i18n-collect.mjs
import fs from 'node:fs';

const PORT = process.env.CDP_PORT ?? '9336';
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

let targets;
for (let i = 0; i < 40; i++) {
  try {
    targets = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
    if (targets?.length) break;
  } catch { /* wait */ }
  await sleep(300);
}
const page = targets.find((t) => t.type === 'page') ?? targets[0];
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((res, rej) => {
  ws.addEventListener('open', res);
  ws.addEventListener('error', rej);
});
let id = 0;
const pending = new Map();
ws.addEventListener('message', (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
  }
});
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const mid = ++id;
    pending.set(mid, { resolve, reject });
    ws.send(JSON.stringify({ id: mid, method, params }));
  });
await send('Runtime.enable');

async function evalJs(expr) {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.text);
  return r.result?.value;
}

const COLLECT = `(()=>{const out=new Set();const tw=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);let n=tw.currentNode;while(n){const v=(n.nodeValue||'').trim();if(/[\\u4e00-\\u9fff]/.test(v))out.add(v);n=tw.nextNode();}return [...out];})()`;
const ATTR_COLLECT = `(()=>{const out=new Set();for(const a of ['title','aria-label','placeholder','alt']){for(const e of document.querySelectorAll('['+a+']')){const v=(e.getAttribute(a)||'').trim();if(/[\\u4e00-\\u9fff]/.test(v))out.add(v);}}return [...out];})()`;

const all = new Set();
async function grab(label) {
  const [texts, attrs] = await Promise.all([evalJs(COLLECT), evalJs(ATTR_COLLECT)]);
  texts.forEach((x) => all.add(x));
  attrs.forEach((x) => all.add(x));
  console.log(label, '累计', all.size);
}
async function click(text) {
  await evalJs(`(()=>{const b=[...document.querySelectorAll('button')].find(x=>x.textContent.trim().includes(${JSON.stringify(text)}));if(b)b.click();return !!b;})()`);
  await sleep(350);
}
async function clickExact(text) {
  await evalJs(`(()=>{const b=[...document.querySelectorAll('button')].find(x=>x.textContent.trim()===${JSON.stringify(text)});if(b)b.click();return !!b;})()`);
  await sleep(350);
}

await send('Page.reload', { ignoreCache: true });
await sleep(1400);
await grab('引导 + 初始页');

await click('开始使用');
await grab('引导关闭后');

for (const [name, label] of [
  ['overview', '总览'],
  ['library', '游戏库'],
  ['versions', '版本'],
  ['tuning', '画面调校'],
  ['logs', '日志诊断'],
]) {
  await evalJs(`(()=>{const b=[...document.querySelectorAll('aside .nav-item')].find(x=>x.textContent.trim()===${JSON.stringify(label)});if(b)b.click();return !!b;})()`);
  await sleep(400);
  await grab('页面 ' + name);
}

// 回到版本页打开两个新弹窗
await evalJs(`(()=>{const b=[...document.querySelectorAll('aside .nav-item')].find(x=>x.textContent.trim()==='版本');if(b)b.click();})()`);
await sleep(350);
await click('导入本地包');
await grab('导入包弹窗');
await evalJs(`(()=>{const o=document.querySelector('.anim-overlay');if(o)o.click();})()`);
await sleep(300);
await click('手动标记');
await grab('手动标记弹窗');
await evalJs(`(()=>{const o=document.querySelector('.anim-overlay');if(o)o.click();})()`);
await sleep(300);

// 设置弹窗各分类
await clickExact('设置');
for (const s of ['外观', '下载', '安全', '通用', '关于']) {
  await evalJs(`(()=>{const b=[...document.querySelectorAll('.anim-panel .nav-item')].find(x=>x.textContent.trim()===${JSON.stringify(s)});if(b)b.click();return !!b;})()`);
  await sleep(300);
  await grab('设置 ' + s);
}
await evalJs(`(()=>{const b=document.querySelector('.anim-panel button[aria-label]');if(b)b.click();})()`);
await sleep(300);

// 命令面板
await click('搜索游戏或操作');
await grab('命令面板');
await evalJs(`(()=>{document.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));})()`);

const out = [...all].sort();
fs.writeFileSync(new URL('./_collected.json', import.meta.url), JSON.stringify(out, null, 2), 'utf8');
const dict = JSON.parse(fs.readFileSync(new URL('../src/i18n/en.json', import.meta.url), 'utf8'));
const missing = out.filter((x) => !(x in dict));
fs.writeFileSync(new URL('./_missing.json', import.meta.url), JSON.stringify(missing, null, 2), 'utf8');
console.log('总片段', out.length, '字典缺失', missing.length);
ws.close();
