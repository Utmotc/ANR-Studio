// 通过 WebView2 远程调试端口截图：node scripts/shot-cdp.mjs <out.png> [width] [height]
const [, , out = 'shot.png', w = '1280', h = '840'] = process.argv;

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
  let targets;
  for (let i = 0; i < 30; i++) {
    try {
      const res = await fetch('http://127.0.0.1:9222/json');
      targets = await res.json();
      if (targets.length) break;
    } catch {
      /* 还没起来 */
    }
    await sleep(400);
  }
  if (!targets?.length) throw new Error('没有找到调试目标，应用可能没带调试端口启动');

  const page = targets.find((t) => t.type === 'page') ?? targets[0];
  const ws = new WebSocket(page.webSocketDebuggerUrl);

  let id = 0;
  const pending = new Map();
  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const mid = ++id;
      pending.set(mid, { resolve, reject });
      ws.send(JSON.stringify({ id: mid, method, params }));
    });

  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve);
    ws.addEventListener('error', reject);
  });

  ws.addEventListener('message', (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      msg.error ? reject(new Error(msg.error.message)) : resolve(msg.result);
    }
  });

  await send('Page.enable');
  await send('Emulation.setDeviceMetricsOverride', {
    width: Number(w),
    height: Number(h),
    deviceScaleFactor: 1,
    mobile: false,
  });
  await sleep(1200);

  const shot = await send('Page.captureScreenshot', { format: 'png' });
  const { writeFileSync } = await import('node:fs');
  writeFileSync(out, Buffer.from(shot.data, 'base64'));
  console.log('saved', out);
  ws.close();
}

main().catch((e) => {
  console.error('FAIL:', e.message);
  process.exit(1);
});
