// 抓页面 JS 异常：node scripts/cdp-errors.mjs [seconds]
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const secs = Number(process.argv[2] ?? 8);

const res = await fetch('http://127.0.0.1:' + (process.env.CDP_PORT ?? '9222') + '/json');
const targets = await res.json();
const page = targets.find((t) => t.type === 'page') ?? targets[0];
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((ok, no) => {
  ws.addEventListener('open', ok);
  ws.addEventListener('error', no);
});

let id = 0;
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const mid = ++id;
    const onMsg = (ev) => {
      const m = JSON.parse(ev.data);
      if (m.id === mid) {
        ws.removeEventListener('message', onMsg);
        m.error ? reject(new Error(m.error.message)) : resolve(m.result);
      }
    };
    ws.addEventListener('message', onMsg);
    ws.send(JSON.stringify({ id: mid, method, params }));
  });

ws.addEventListener('message', (ev) => {
  const m = JSON.parse(ev.data);
  if (m.method === 'Runtime.exceptionThrown') {
    const d = m.params.exceptionDetails;
    console.log('[exception]', d.text, d.exception?.description ?? '', `${d.url}:${d.lineNumber}`);
  }
  if (m.method === 'Runtime.consoleAPICalled' && ['error', 'warning'].includes(m.params.type)) {
    console.log('[console.' + m.params.type + ']', m.params.args.map((a) => a.value ?? a.description).join(' '));
  }
});

await send('Runtime.enable');
await send('Page.enable');
await send('Page.reload', { ignoreCache: true });
await sleep(secs * 1000);
process.exit(0);
