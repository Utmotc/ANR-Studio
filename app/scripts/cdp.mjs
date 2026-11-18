// 通过 WebView2 远程调试端口操作界面
//   node scripts/cdp.mjs shot out.png [w] [h]
//   node scripts/cdp.mjs eval "js expression"
const [, , mode, ...rest] = process.argv;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function connect() {
  let targets;
  for (let i = 0; i < 40; i++) {
    try {
      const res = await fetch('http://127.0.0.1:' + (process.env.CDP_PORT ?? '9222') + '/json');
      targets = await res.json();
      if (targets?.length) break;
    } catch {
      /* 等待应用启动 */
    }
    await sleep(400);
  }
  if (!targets?.length) throw new Error('没有找到调试目标');

  const page = targets.find((t) => t.type === 'page') ?? targets[0];
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve);
    ws.addEventListener('error', reject);
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
  return { send, close: () => ws.close() };
}

async function main() {
  const { send, close } = await connect();

  if (mode === 'shot') {
    const [out = 'shot.png', w = '1280', h = '840'] = rest;
    await send('Emulation.setDeviceMetricsOverride', {
      width: Number(w),
      height: Number(h),
      deviceScaleFactor: 1,
      mobile: false,
    });
    await sleep(600);
    const shot = await send('Page.captureScreenshot', { format: 'png' });
    const { writeFileSync } = await import('node:fs');
    writeFileSync(out, Buffer.from(shot.data, 'base64'));
    console.log('saved', out);
  } else if (mode === 'eval') {
    const expr = rest.join(' ');
    const r = await send('Runtime.evaluate', {
      expression: expr,
      awaitPromise: true,
      returnByValue: true,
    });
    if (r.exceptionDetails) {
      console.error('JS ERROR:', r.exceptionDetails.text, r.exceptionDetails.exception?.description);
    } else {
      console.log(JSON.stringify(r.result?.value ?? r.result));
    }
  } else {
    throw new Error('用法: cdp.mjs shot <out.png> | cdp.mjs eval "<js>"');
  }

  close();
}

main().catch((e) => {
  console.error('FAIL:', e.message);
  process.exit(1);
});
