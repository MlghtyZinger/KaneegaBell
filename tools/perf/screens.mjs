// Resolution / DPI test matrix: screenshots of every main screen at each
// display size and Windows scaling level, plus automatic layout checks
// (sideways page scroll, content pushed off screen, clipped text, tiny text).
//
//   node tools/perf/screens.mjs <port> [outDir] [--only 1920x1080@1,...] [--states library,tv] [--no-zoom]
//
// <port> is a DevTools port: KB started with
// WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>, or a
// browser showing the mock UI (npm run dev, http://localhost:1420/?art=1).
// Writes <state>-<W>x<H>@<scale>.jpg, report.json and index.html to outDir
// (default perf-lib/screens).
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const args = process.argv.slice(2);
const flag = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : null;
};
const only = flag("--only")?.split(",");
const onlyStates = flag("--states")?.split(",");
const noZoom = args.includes("--no-zoom") && args.splice(args.indexOf("--no-zoom"), 1);
const [port = "9333", outDir = "perf-lib/screens"] = args;
mkdirSync(outDir, { recursive: true });

// Physical display sizes x Windows scaling. The page sees size / scale CSS
// pixels; combinations below KB's minimum window (960x540) can't happen.
const DISPLAYS = [
  [960, 540, "smallest window"],
  [1280, 720, "720p"],
  [1600, 900, "900p"],
  [1920, 1080, "1080p"],
  [2560, 1440, "1440p"],
  [3440, 1440, "21:9 ultrawide"],
  [5120, 1440, "32:9 super ultrawide"],
  [3840, 2160, "4K"],
];
const SCALES = [1, 1.5, 2, 3];
// KB's "Fit the screen" page zoom (display.rs auto_zoom): bigger than
// 1080p-equivalent lays out like 1080p. --no-zoom shows "Windows scaling only".
const autoZoom = (w, h) => (noZoom ? 1 : Math.round(Math.max(1, Math.min(h / 1080, w / 1920)) * 100) / 100);
const MATRIX = DISPLAYS.flatMap(([w, h, name]) =>
  SCALES.map((s) => {
    const z = autoZoom(w / s, h / s);
    return { w, h, s, z, name, cw: Math.round(w / s / z), ch: Math.round(h / s / z) };
  }).filter((m) => Math.round(m.w / m.s) >= 960 && Math.round(m.h / m.s) >= 540),
).filter((m) => !only || only.includes(`${m.w}x${m.h}@${m.s}`));

// ------------------------------------------------------------------ DevTools
const pages = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).filter((t) => t.type === "page");
const page = pages.find((t) => t.title === "KaneegaBell") ?? pages[0];
if (!page) throw new Error(`no page on DevTools port ${port}`);
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
let nextId = 0;
const pending = new Map();
ws.addEventListener("message", (ev) => {
  const m = JSON.parse(ev.data);
  if (m.id && pending.has(m.id)) {
    pending.get(m.id)(m);
    pending.delete(m.id);
  }
});
const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const n = ++nextId;
    pending.set(n, (m) => (m.error ? reject(new Error(`${method}: ${m.error.message}`)) : resolve(m.result)));
    ws.send(JSON.stringify({ id: n, method, params }));
  });
const evaluate = async (expression) => {
  const r = await send("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? "evaluate failed");
  return r.result.value;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const KEYS = { Enter: 13, Escape: 27, F1: 112, F10: 121, e: 69 };
async function press(key) {
  const base = { key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, windowsVirtualKeyCode: KEYS[key] };
  await send("Input.dispatchKeyEvent", { type: "keyDown", ...base, ...(key.length === 1 ? { text: key } : {}) });
  await send("Input.dispatchKeyEvent", { type: "keyUp", ...base });
  await sleep(250);
}
const click = (text) =>
  evaluate(`(() => { const b = [...document.querySelectorAll('button')].find((b) => b.textContent.includes(${JSON.stringify(text)})); b?.click(); return !!b; })()`);
const waitFor = async (selector, ms = 4000) => {
  for (const t0 = Date.now(); Date.now() - t0 < ms; await sleep(100)) {
    if (await evaluate(`!!document.querySelector(${JSON.stringify(selector)})`)) return true;
  }
  throw new Error(`timed out waiting for ${selector}`);
};

// ------------------------------------------------------------------ checks
// Runs in the page. Content inside a scroll container or a clipping parent
// is allowed past the edge (that's how the grid, shelf and lists work);
// anything else past the window's edge is a problem.
const CHECKS = `(() => {
  const W = innerWidth, H = innerHeight, out = [];
  const name = (el) => el.tagName.toLowerCase() + (el.classList.length ? '.' + [...el.classList].slice(0, 2).join('.') : '');
  const clipped = (el) => {
    for (let p = el.parentElement; p && p !== document.body; p = p.parentElement) {
      const cs = getComputedStyle(p);
      if (cs.overflowX !== 'visible' || cs.overflowY !== 'visible') return true;
    }
    return false;
  };
  const de = document.documentElement;
  if (de.scrollWidth > W + 1) out.push({ type: 'page scrolls sideways', el: 'html', detail: de.scrollWidth + ' > ' + W });
  let minFont = Infinity, minFontEl = '';
  for (const el of document.querySelectorAll('body *')) {
    const cs = getComputedStyle(el);
    if (cs.display === 'none' || cs.visibility === 'hidden' || +cs.opacity === 0) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1) continue;
    const ownText = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
    if (ownText && r.bottom > 0 && r.top < H && r.right > 0 && r.left < W) {
      const f = parseFloat(cs.fontSize);
      if (f < minFont) { minFont = f; minFontEl = name(el); }
    }
    if (cs.position === 'fixed' && r.width >= W - 1 && r.height >= H - 1) continue; // overlays, backgrounds
    if (!clipped(el) && (r.right > W + 1 || r.bottom > H + 1 || r.left < -1)) {
      out.push({ type: 'past the window edge', el: name(el), detail: [r.left, r.top, r.right, r.bottom].map(Math.round).join(',') });
    }
    if (ownText && el.scrollWidth > el.clientWidth + 1 && cs.overflowX !== 'visible' && cs.textOverflow !== 'ellipsis') {
      out.push({ type: 'text cut off', el: name(el), detail: el.textContent.trim().slice(0, 40) });
    }
  }
  const seen = new Set();
  const issues = out.filter((i) => { const k = i.type + i.el; if (seen.has(k)) return false; seen.add(k); return true; });
  return { issues, minFontCss: minFont, minFontEl, bodyFontCss: parseFloat(getComputedStyle(document.body).fontSize) };
})()`;

// ------------------------------------------------------------------ states
// Each state starts from the library with nothing open.
const toLibrary = async () => {
  for (let i = 0; i < 4 && !(await evaluate("!!document.querySelector('.vgrid') && !document.querySelector('.overlay, .settings, .tv')")); i++) {
    await press("Escape");
  }
  if (await evaluate("!!document.querySelector('.tv')")) await press("F10");
};
const STATES = [
  ["library", async () => waitFor(".vgrid-cell")],
  ["details", async () => { await press("Enter"); await waitFor(".info-panel"); await sleep(300); }],
  ["settings-library", async () => { await press("F1"); await waitFor(".settings"); }],
  ["settings-platforms", async () => { await press("F1"); await waitFor(".settings"); await click("Platforms"); await sleep(200); }],
  ["settings-general", async () => {
    await press("F1"); await waitFor(".settings"); await click("General"); await sleep(300);
    await evaluate("[...document.querySelectorAll('h2')].find((h) => h.textContent === 'TV Mode')?.scrollIntoView({ block: 'center' }), 1");
    await sleep(200);
  }],
  ["settings-dialog", async () => { await press("F1"); await waitFor(".settings"); await click("General"); await sleep(200); await click("Reset KaneegaBell"); await waitFor(".confirm-panel"); }],
  ["tv", async () => { await press("F10"); await waitFor(".tv"); await sleep(900); }],
].filter(([s]) => !onlyStates || onlyStates.includes(s));

// ------------------------------------------------------------------ run
const results = [];
for (const m of MATRIX) {
  await send("Emulation.setDeviceMetricsOverride", { width: m.cw, height: m.ch, deviceScaleFactor: m.s * m.z, mobile: false });
  await sleep(300);
  for (const [state, open] of STATES) {
    const label = `${state}-${m.w}x${m.h}@${m.s}`;
    try {
      await toLibrary();
      await open();
      await sleep(250);
      const check = await evaluate(CHECKS);
      const shot = await send("Page.captureScreenshot", { format: "jpeg", quality: 80 });
      writeFileSync(join(outDir, `${label}.jpg`), Buffer.from(shot.data, "base64"));
      results.push({ state, display: m.name, w: m.w, h: m.h, scale: m.s, zoom: m.z, css: `${m.cw}x${m.ch}`, file: `${label}.jpg`, ...check });
      const n = check.issues.length;
      console.log(`${label}: ${n ? n + " issue(s)" : "ok"} (smallest text ${check.minFontCss}px css = ${Math.round(check.minFontCss * m.s * m.z)}px on screen)`);
    } catch (e) {
      results.push({ state, display: m.name, w: m.w, h: m.h, scale: m.s, css: `${m.cw}x${m.ch}`, error: String(e.message ?? e) });
      console.log(`${label}: FAILED ${e.message ?? e}`);
    }
  }
}
await toLibrary();
await send("Emulation.clearDeviceMetricsOverride");
ws.close();

writeFileSync(join(outDir, "report.json"), JSON.stringify(results, null, 2));
const esc = (s) => String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
const rows = results
  .map(
    (r) => `<figure class="${r.error || r.issues?.length ? "bad" : ""}">
  ${r.file ? `<a href="${r.file}"><img loading="lazy" src="${r.file}"></a>` : ""}
  <figcaption><b>${esc(r.state)}</b> · ${r.w}×${r.h} @ ${r.scale * 100}% (${esc(r.display)}, ${r.css} css${r.zoom > 1 ? `, page zoom ${r.zoom}` : ""})
  ${r.error ? `<div class="err">${esc(r.error)}</div>` : ""}
  ${r.issues?.length ? `<ul>${r.issues.slice(0, 8).map((i) => `<li>${esc(i.type)}: <code>${esc(i.el)}</code> ${esc(i.detail ?? "")}</li>`).join("")}${r.issues.length > 8 ? `<li>…${r.issues.length - 8} more</li>` : ""}</ul>` : ""}
  ${r.minFontCss ? `<div class="dim">smallest text ${r.minFontCss}px (${esc(r.minFontEl)}), ${Math.round(r.minFontCss * r.scale * (r.zoom ?? 1))}px on screen</div>` : ""}
  </figcaption></figure>`,
  )
  .join("\n");
writeFileSync(
  join(outDir, "index.html"),
  `<!doctype html><meta charset="utf-8"><title>KB resolution matrix</title>
<style>body{background:#0e1014;color:#eef0f4;font:14px system-ui;margin:16px}main{display:grid;grid-template-columns:repeat(auto-fill,minmax(420px,1fr));gap:16px}
figure{margin:0;background:#171a21;border:1px solid #2c313c;border-radius:8px;padding:8px}figure.bad{border-color:#ff6b6b}img{width:100%;display:block;border-radius:4px}
figcaption{margin-top:6px}ul{margin:4px 0;padding-left:18px;color:#ffb3b3}.err{color:#ff6b6b}.dim{color:#a3a9b6}code{color:#ffd75e}</style>
<h1>KB resolution / scaling matrix</h1><p>${results.length} screenshots, ${results.filter((r) => r.error || r.issues?.length).length} with problems (red).</p><main>${rows}</main>`,
);
const bad = results.filter((r) => r.error || r.issues?.length).length;
console.log(`\n${results.length} screenshots, ${bad} with problems: ${join(outDir, "index.html")}`);
