// 在已经切到英文的界面上遍历各页面/弹窗，列出仍然残留中文的文本片段。
// 用法：CDP_PORT=9336 node scripts/i18n-verify.mjs
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
await new Promise((res) => ws.addEventListener('open', res));
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
const evalJs = async (expr) => {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  return r.result?.value;
};
const COLLECT = `(()=>{const s=new Set();const tw=document.createTreeWalker(document.body,NodeFilter.SHOW_TEXT);let n=tw.currentNode;while(n){const v=(n.nodeValue||'').trim();if(/[\\u4e00-\\u9fff]/.test(v))s.add(v);n=tw.nextNode();}for(const a of ['title','aria-label','placeholder']){for(const e of document.querySelectorAll('['+a+']')){const v=(e.getAttribute(a)||'').trim();if(/[\\u4e00-\\u9fff]/.test(v))s.add(v);}}return [...s];})()`;

const all = new Set();
const grab = async (label) => {
  (await evalJs(COLLECT)).forEach((x) => all.add(x));
  console.log(label, '累计残留', all.size);
};
const clickText = async (text, sel = 'button') => {
  await evalJs(`(()=>{const b=[...document.querySelectorAll(${JSON.stringify(sel)})].find(x=>x.textContent.trim()===${JSON.stringify(text)});if(b)b.click();return !!b;})()`);
  await sleep(350);
};

await grab('当前页');
for (const label of ['Overview', 'Library', 'Versions', 'Picture tuning', 'Logs & diagnostics']) {
  await clickText(label, 'aside .nav-item');
  await grab('页面 ' + label);
}
await clickText('Versions', 'aside .nav-item');
for (const label of ['Import local package', 'Mark version']) {
  await clickText(label);
  await grab('弹窗 ' + label);
  await evalJs(`(()=>{const o=document.querySelector('.anim-overlay');if(o)o.click();})()`);
  await sleep(300);
}
await clickText('Settings', 'aside .nav-item');
for (const s of ['Appearance', 'Download', 'Safety', 'General', 'About']) {
  await clickText(s, '.anim-panel .nav-item');
  await grab('设置 ' + s);
}
await evalJs(`(()=>{const b=document.querySelector('.anim-panel button[aria-label]');if(b)b.click();})()`);
await sleep(300);

console.log('\n残留中文片段:');
for (const x of [...all].sort()) console.log(' |', x);
console.log('合计', all.size);
ws.close();
