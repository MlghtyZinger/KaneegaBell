// Focused TV regression checks on an isolated KB perf instance, never the owner's library.
// node tools/perf/console-menus.mjs <DevTools port> <output directory>
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [port, output] = process.argv.slice(2);
if (!port || !output) throw new Error("usage: console-menus.mjs <port> <output directory>");
const pages = await (await fetch(`http://127.0.0.1:${Number(port)}/json`)).json();
const page = pages.find((item) => item.type === "page" && item.title === "KaneegaBell");
if (!page) throw new Error("isolated KaneegaBell page not found");
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve) => ws.addEventListener("open", resolve, { once: true }));
let next = 0;
const pending = new Map();
ws.addEventListener("message", ({ data }) => {
  const message = JSON.parse(data);
  if (!pending.has(message.id)) return;
  const { resolve, reject } = pending.get(message.id);
  pending.delete(message.id);
  if (message.error) reject(new Error(JSON.stringify(message.error)));
  else resolve(message.result);
});
const send = (method, params = {}) => new Promise((resolve, reject) => {
  const id = ++next;
  pending.set(id, { resolve, reject });
  ws.send(JSON.stringify({ id, method, params }));
});
const evaluate = async (expression) => {
  const result = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.exception?.description ?? "page evaluation failed");
  return result.result.value;
};
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function wait(selector) {
  for (let i = 0; i < 100; i++) {
    if (await evaluate(`!!document.querySelector(${JSON.stringify(selector)})`)) return;
    await sleep(100);
  }
  throw new Error(`timed out waiting for ${selector}`);
}
async function press(key) {
  const code = { F10: 121, Escape: 27, Enter: 13, ArrowDown: 40, ArrowRight: 39 }[key];
  await send("Input.dispatchKeyEvent", { type: "keyDown", key, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key, windowsVirtualKeyCode: code, nativeVirtualKeyCode: code });
}
async function style(name) {
  await evaluate("document.querySelector('.tv-style-switch').click()");
  await wait(".tv-style-picker");
  await evaluate(`Array.from(document.querySelectorAll('.tv-style-picker button')).find((button) => button.textContent.trim().startsWith(${JSON.stringify(name)})).click()`);
  await wait(`.console-${name.toLowerCase()}`);
  await sleep(400);
}
const results = [];
mkdirSync(output, { recursive: true });
try {
  if (!(await evaluate("!!document.querySelector('.tv')"))) await press("F10");
  await wait(".tv-style-switch");
  await style("PS3");
  const effects = await evaluate(`({ sparks:document.querySelectorAll(".xmb-sparkles i").length, duration:getComputedStyle(document.querySelector(".xmb-wave")).animationDuration, musicPaused:document.querySelector(".console-menu-music").paused })`);
  results.push({ test:"PS3 sparkles, faster waves, no music", ...effects, passed:effects.sparks === 42 && effects.duration === "10s" && effects.musicPaused });
  for (const [width, height] of [[1280, 720], [1920, 1080], [2560, 1440], [3840, 2160]]) {
    await send("Emulation.setDeviceMetricsOverride", { width, height, deviceScaleFactor: 1, mobile: false });
    await sleep(300);
    for (const target of [0, 3, -1, 0]) {
      // Native focus is intentional: this reproduced scrolling of the old overflow:hidden screen.
      await evaluate(`(() => { const buttons = document.querySelectorAll('.xmb-category'); const button = buttons[${target} < 0 ? buttons.length - 1 : ${target}]; button.focus(); button.scrollIntoView({block:'nearest',inline:'nearest'}); button.click(); })()`);
      await sleep(350);
      const result = await evaluate(`(() => {
        const surface = document.querySelector('.console-ps3');
        const bar = document.querySelector('.xmb-category.active .icon').getBoundingClientRect();
        const thumb = document.querySelector('.xmb-item.active .xmb-thumb')?.getBoundingClientRect();
        const controls = Array.from(document.querySelectorAll('.xmb-keys > *')).map((element) => { const r = element.getBoundingClientRect(); return {left:r.left,right:r.right,top:r.top,bottom:r.bottom}; });
        return { viewport:[innerWidth,innerHeight], scrollLeft:surface.scrollLeft, scrollTop:surface.scrollTop,
          parentScrollLeft:surface.parentElement.scrollLeft, iconHeight:bar.height,
          categoryGameGap:thumb ? thumb.top-bar.bottom : 0,
          renderedGames:document.querySelectorAll(".xmb-item").length,
          alignmentError:thumb ? Math.abs(bar.left+bar.width/2-thumb.left-thumb.width/2) : 0,
          controlsFit:controls.every((r) => r.left >= 0 && r.right <= innerWidth && r.top >= 0 && r.bottom <= innerHeight),
          popup:!!document.querySelector('.console-platform-notice'),
          wave:getComputedStyle(document.querySelector('.xmb-wave')).animationPlayState };
      })()`);
      result.passed = result.scrollLeft === 0 && result.scrollTop === 0 && result.parentScrollLeft === 0 && result.controlsFit && result.alignmentError < 2 && result.categoryGameGap >= 0 && result.categoryGameGap < height * .14 && result.renderedGames <= 6 && !result.popup && result.iconHeight >= height * .07 && result.wave === "running";
      results.push(result);
    }
    const capture = await send("Page.captureScreenshot", { format: "jpeg", quality: 80 });
    writeFileSync(join(output, `ps3-${width}x${height}.jpg`), Buffer.from(capture.data, "base64"));
  }
  await send("Emulation.setDeviceMetricsOverride", { width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false });
  await style("Wii");
  for (let i = 0; i < 3; i++) await press("ArrowDown");
  const dock = await evaluate("!!document.activeElement?.closest('.wii-dock')");
  await press("ArrowRight"); await press("ArrowRight"); await press("Enter");
  await wait(".tv-style-picker");
  results.push({ test: "Wii dock reachable with Down, Right, A/Enter", passed: dock });
  await press("Escape");
  if (process.env.KB_TEST_MENU_MUSIC === "1") {
    await sleep(1200);
    const music = await evaluate('(() => { const m=document.querySelector(".console-menu-music"); return {paused:m.paused, loop:m.loop, duration:m.duration, time:m.currentTime, volume:m.volume, error:m.error?.message}; })()');
    results.push({test:"Wii theme decodes and plays on loop", ...music, passed:!music.paused && music.loop && music.time > 0 && Math.abs(music.duration-74.208)<.1 && music.volume>0});
    await evaluate('(() => { const m=document.querySelector(".console-menu-music"); m.currentTime=m.duration-.15; })()');
    await sleep(900);
    results.push({test:"Wii theme loops at its actual end", passed:await evaluate('(() => { const m=document.querySelector(".console-menu-music"); return !m.paused && m.currentTime<2; })()')});
    await style("PS3");
    results.push({test:"Wii music pauses in PS3", passed:await evaluate('document.querySelector(".console-menu-music").paused')});
    await style("Wii");
    await sleep(300);
    results.push({test:"Wii music resumes on return", passed:await evaluate('!document.querySelector(".console-menu-music").paused')});
    await evaluate('window.__qaMusic=document.querySelector(".console-menu-music")');
    await press("F10"); await sleep(300);
    results.push({test:"Music stops and releases source on leaving TV", passed:await evaluate('window.__qaMusic.paused && !window.__qaMusic.hasAttribute("src")')});
    await press("F10"); await wait(".console-wii");
  }
  const capture = await send("Page.captureScreenshot", { format: "jpeg", quality: 80 });
  writeFileSync(join(output, "wii-dock-1920x1080.jpg"), Buffer.from(capture.data, "base64"));
  writeFileSync(join(output, "report.json"), JSON.stringify(results, null, 2) + "\n");
  console.log(JSON.stringify({ checks: results.length, passed: results.filter((result) => result.passed).length, output }));
  if (results.some((result) => !result.passed)) process.exitCode = 1;
} finally {
  await send("Emulation.clearDeviceMetricsOverride").catch(() => {});
  ws.close();
}
