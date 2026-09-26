// Open the browser's own bookmarks page over the DevTools protocol and keep
// creating bookmarks from it, so the browser keeps rewriting `Bookmarks`.
const [port, page, seconds] = process.argv.slice(2);
const base = `http://127.0.0.1:${port}`;
const target = await (await fetch(`${base}/json/new?${page}`, { method: 'PUT' })).json();
const ws = new WebSocket(target.webSocketDebuggerUrl);
let id = 0;
const send = (method, params = {}) => new Promise((resolve) => {
  const mine = ++id;
  const onMessage = (event) => {
    const msg = JSON.parse(event.data);
    if (msg.id === mine) { ws.removeEventListener('message', onMessage); resolve(msg); }
  };
  ws.addEventListener('message', onMessage);
  ws.send(JSON.stringify({ id: mine, method, params }));
});
await new Promise((r) => ws.addEventListener('open', r));
await new Promise((r) => setTimeout(r, 3000));
const script = `(async () => {
  const end = Date.now() + ${Number(seconds) * 1000};
  let n = 0;
  while (Date.now() < end) {
    n += 1;
    const folder = await chrome.bookmarks.create({parentId: '1', title: 'Folder ' + n});
    for (let i = 0; i < 5; i++) {
      await chrome.bookmarks.create({parentId: folder.id, title: 'Σελίδα ' + n + '.' + i, url: 'https://bm.example/' + n + '/' + i});
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  return n;
})()`;
const result = await send('Runtime.evaluate', { expression: script, awaitPromise: true, returnByValue: true });
console.log(JSON.stringify(result.result?.result ?? result));
ws.close();
