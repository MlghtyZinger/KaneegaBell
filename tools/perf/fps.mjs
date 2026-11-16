// Grid scrolling smoothness, measured inside the app's WebView2 over the
// DevTools protocol: node tools/perf/fps.mjs <port>
// Scrolls the library grid top to bottom at a steady speed (like holding the
// stick) and records every frame's duration. Pass 1 decodes thumbnails for
// the first time; pass 2 scrolls through the same rows again.
const port = process.argv[2] ?? "9333";
const targets = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = targets.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let id = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
});
const send = (method, params = {}) =>
  new Promise((r) => {
    const n = ++id;
    pending.set(n, r);
    ws.send(JSON.stringify({ id: n, method, params }));
  });
const evaluate = async (expr) => {
  const r = await send("Runtime.evaluate", { expression: expr, awaitPromise: true, returnByValue: true });
  if (r.result?.exceptionDetails) throw new Error(r.result.exceptionDetails.exception?.description ?? "eval failed");
  return r.result.result.value;
};

const scrollPass = (pxPerSec, ms) => `(async () => {
  const el = document.querySelector('.vgrid');
  if (!el) throw new Error('grid not found');
  el.scrollTop = 0;
  await new Promise((r) => setTimeout(r, 300));
  const max = el.scrollHeight - el.clientHeight;
  const frames = [];
  let last = 0, start = 0;
  await new Promise((done) => {
    const step = (t) => {
      if (!start) { start = last = t; requestAnimationFrame(step); return; }
      frames.push(t - last); last = t;
      const e = t - start;
      el.scrollTop = Math.min(max, e * ${pxPerSec} / 1000);
      if (e < ${ms} && el.scrollTop < max) requestAnimationFrame(step); else done();
    };
    requestAnimationFrame(step);
  });
  const s = [...frames].sort((a, b) => a - b);
  const q = (p) => s[Math.min(s.length - 1, Math.floor(p * s.length))];
  const total = frames.reduce((a, b) => a + b, 0);
  return {
    frames: frames.length,
    scrolledPx: Math.round(el.scrollTop),
    avgFps: +(1000 * frames.length / total).toFixed(1),
    p50ms: +q(0.5).toFixed(1), p95ms: +q(0.95).toFixed(1), p99ms: +q(0.99).toFixed(1), maxMs: +s[s.length - 1].toFixed(1),
    over20ms: frames.filter((f) => f > 20).length,
    over33ms: frames.filter((f) => f > 33.4).length,
  };
})()`;

const info = await evaluate(`({
  games: document.querySelector('.vgrid-sizer') ? 'ok' : 'none',
  viewport: innerWidth + 'x' + innerHeight + ' @' + devicePixelRatio + 'x',
  cellsInDom: document.querySelectorAll('.vgrid-cell').length,
  imgsInDom: document.querySelectorAll('.vgrid img').length,
  gridHeightPx: document.querySelector('.vgrid-sizer')?.offsetHeight,
})`);
console.log("grid:", JSON.stringify(info));
const passes = {};
for (const [key, label, speed] of [["first", "pass 1 (thumbnails decoded for the first time)", 2500], ["second", "pass 2 (same rows again)", 2500], ["flick", "fast flick (6000 px/s)", 6000]]) {
  passes[key] = await evaluate(scrollPass(speed, 15000));
  console.log(`${label}:`, JSON.stringify(passes[key]));
}
await send("Performance.enable");
const m = Object.fromEntries((await send("Performance.getMetrics")).result.metrics.map((x) => [x.name, x.value]));
console.log(
  "page:",
  JSON.stringify({
    domNodes: m.Nodes,
    jsHeapUsedMB: +(m.JSHeapUsedSize / 1048576).toFixed(1),
    cellsInDomAfter: await evaluate("document.querySelectorAll('.vgrid-cell').length"),
  }),
);
// One machine-readable line for perf.ps1's regression gate.
console.log("RESULT " + JSON.stringify(passes));
ws.close();
