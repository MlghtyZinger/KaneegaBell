// One DevTools-protocol call into the app's WebView2 (used by perf.ps1):
// node cdp.mjs <port> eval <expression>   |   node cdp.mjs <port> <Method> <json-params>
// Fail instead of hanging (rAF stops in a hidden/suspended window, a reply can never come).
setTimeout(() => { console.error("cdp.mjs: the DevTools call took over 30 s, giving up"); process.exit(2); }, 30000).unref();
const [port, method, arg] = process.argv.slice(2);
const page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r, j) => { ws.addEventListener("open", r, { once: true }); ws.addEventListener("error", () => j(new Error("DevTools connection failed")), { once: true }); });
const send = (m, params) =>
  new Promise((r) => {
    ws.addEventListener("message", (ev) => { const d = JSON.parse(ev.data); if (d.id === 1) r(d); });
    ws.send(JSON.stringify({ id: 1, method: m, params }));
  });
const res = method === "eval"
  ? await send("Runtime.evaluate", { expression: arg, awaitPromise: true, returnByValue: true })
  : await send(method, JSON.parse(arg ?? "{}"));
console.log(JSON.stringify(res.result?.result?.value ?? res.result ?? res.error));
ws.close();
