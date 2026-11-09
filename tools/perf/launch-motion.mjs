// Integration check against the generated library's stand-in PS2 emulator only.
// Start KB with KB_DATA_DIR=perf-lib/lib5000/data and remote debugging enabled,
// then: node tools/perf/launch-motion.mjs <port>. Never use a personal library.
import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
const port = process.argv[2] ?? '9333';
const page = (await (await fetch(`http://127.0.0.1:${port}/json`)).json()).find(t => t.type === 'page');
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve, reject) => { ws.onopen = resolve; ws.onerror = reject; });
let id = 0;
const pending = new Map();
ws.onmessage = ({ data }) => {
  const result = JSON.parse(data);
  if (pending.has(result.id)) { pending.get(result.id)(result); pending.delete(result.id); }
};
const send = (method, params) => new Promise(resolve => {
  const next = ++id; pending.set(next, resolve); ws.send(JSON.stringify({ id: next, method, params }));
});
const evaluate = async expression => {
  const result = await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
  if (result.result?.exceptionDetails) throw Error(result.result.exceptionDetails.exception?.description ?? 'evaluation failed');
  return result.result.result.value;
};
const wait = ms => new Promise(resolve => setTimeout(resolve, ms));
const invoke = (command, args = {}) => evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},${JSON.stringify(args)})`);
const timeout = setTimeout(() => { console.error('Launch check timed out'); process.exit(1); }, 60000);
try {
  const lib = await invoke('get_library');
  assert.equal(lib.games.length, 5000, 'requires the generated 5,000-game library');
  if (await invoke('get_running')) { await invoke('exit_game'); await wait(5500); }
  for (const [key, value] of Object.entries({introDesktop:'off',introTv:'off',animDesktop:'full',animTv:'full',launchFxDesktop:true,startInTv:false,consoleFeedbackReady:true,autoUpdateCheck:false,railMode:'open'})) {
    await invoke('set_setting', {key,value});
  }
  await send('Page.reload', {});
  await wait(1500);
  let game = lib.games.find(g => g.platformId === 'ps2');
  assert.ok(game);
  await evaluate(`(() => {const rows=Array.from(document.querySelectorAll('.rail-row'));if(!rows.some(e=>e.querySelector('.rail-name')?.textContent==='PlayStation 2'))rows.find(e=>e.querySelector('.rail-name')?.textContent==='PlayStation').click();})()`);
  await wait(100);
  await evaluate(`Array.from(document.querySelectorAll('.rail-row')).find(e=>e.querySelector('.rail-name')?.textContent==='PlayStation 2').click()`);
  await wait(600);
  await evaluate(`document.querySelector('.vgrid-cell').dispatchEvent(new MouseEvent('dblclick',{bubbles:true}))`);
  await wait(700);
  assert.ok(await evaluate(`!!document.querySelector('.dp-cover')`));
  const start = Date.now();
  await evaluate(`document.querySelector('.dp-side .primary').click()`);
  await wait(350);
  const running = await invoke('get_running');
  game = lib.games.find(g=>g.id===running?.gameId);
  assert.equal(game?.platformId, 'ps2', 'game starts before the animation ends');
  assert.ok(await evaluate(`!!document.querySelector('.launch-fx')`), 'animation survives game-started');
  assert.equal(await invoke('plugin:window|is_minimized', { label: 'main' }), false, 'KB stays visible');
  await wait(500);
  const retained = await evaluate(`({visible:!!document.querySelector('.launch-fx'),elapsed:performance.now()})`);
  assert.ok(retained.visible, 'full animation is not cut short');
  await send('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  await send('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape', windowsVirtualKeyCode: 27 });
  await wait(350);
  assert.equal(await evaluate(`!!document.querySelector('.launch-fx')`), false, 'Back skips the presentation');
  assert.equal((await invoke('get_running'))?.gameId, game.id, 'Back keeps the game running');
  assert.equal(await invoke('plugin:window|is_minimized', { label: 'main' }), true, 'skip hands over to the emulator');
  assert.ok(Date.now() - start < 2400, 'skip completes before the normal timeline');
  await invoke('exit_game');
  await wait(5500);
  console.log('PASS: immediate spawn, animation retained, visible presentation, Back skips without cancelling, minimized handoff.');
  await evaluate(`document.querySelector('.vgrid-cell').dispatchEvent(new MouseEvent('dblclick',{bubbles:true}))`);
  await wait(700);
  await evaluate(`document.querySelector('.dp-side .primary').click()`);
  await wait(350);
  assert.ok(await evaluate(`!!document.querySelector('.launch-fx')`));
  assert.equal((await invoke('get_running'))?.gameId, game.id);
  await wait(2350);
  assert.equal(await evaluate(`!!document.querySelector('.launch-fx')`), false, 'normal animation completes');
  assert.equal(await invoke('plugin:window|is_minimized', {label:'main'}), true);
  await invoke('exit_game');
  await wait(5500);
  console.log('PASS: unskipped animation completes and hands over.');
  await invoke('set_setting', {key:'tvMenu',value:'ps2'});
  await invoke('set_setting', {key:'startInTv',value:true});
  await send('Page.reload', {});
  await wait(1800);
  // Keep the unique PS2 game selected through the persisted search.
  await evaluate(`document.querySelector('.ps2-home button').click()`);
  await wait(350);
  for (let i=0;i<20;i++) {
    if (await evaluate(`/(PlayStation 2|PS2)/i.test(document.querySelector('.ps2-tabs .active')?.textContent??'')`)) break;
    await send('Input.dispatchKeyEvent', {type:'keyDown',key:'e',code:'KeyE',windowsVirtualKeyCode:69});
    await send('Input.dispatchKeyEvent', {type:'keyUp',key:'e',code:'KeyE',windowsVirtualKeyCode:69});
    await wait(90);
  }
  await evaluate(`document.querySelector('.ps2-copy button').click()`);
  await wait(350);
  await evaluate(`document.querySelector('.ps2-options button').click()`);
  await wait(200);
  assert.ok(await invoke('get_running'), 'PS2 spawns at Play, before its 1.1s transition');
  assert.ok(await evaluate(`!!document.querySelector('.ps2-runtime.starting')`));
  await wait(1100);
  assert.equal(await evaluate(`!!document.querySelector('.ps2-runtime')`), false);
  await invoke('exit_game');
  await wait(5500);
  await invoke('set_setting', {key:'startInTv',value:false});
  await invoke('set_setting', {key:'tvMenu',value:'kb'});
  console.log('PASS: PS2 spawns immediately and retains its own full launch transition.');
  await send('Page.reload', {});
  await wait(1600);
  const frames = [];
  async function measure(label, action) {
    const result = await evaluate(`(async()=>{
      const samples=[];let last=0,start=0;
      await new Promise(done=>{const frame=t=>{if(!start){start=last=t;${action}}else{samples.push(t-last);last=t;}if(t-start<1000)requestAnimationFrame(frame);else done()};requestAnimationFrame(frame)});
      samples.sort((a,b)=>a-b);return {frames:samples.length,p99Ms:samples[Math.floor(samples.length*.99)],maxMs:samples.at(-1)};
    })()`);
    frames.push({label,...result});
    console.log(label, JSON.stringify(result));
  }
  await evaluate(`Array.from(document.querySelectorAll('.rail-row')).find(e=>e.querySelector('.rail-name')?.textContent==='PlayStation 2').click()`);
  await wait(400);
  await measure('Full platform switch', `document.querySelectorAll('.rail-row')[0].click();`);
  await measure('Full details open', `document.querySelector('.vgrid-cell').dispatchEvent(new MouseEvent('dblclick',{bubbles:true}));`);
  await send('Input.dispatchKeyEvent',{type:'keyDown',key:'Escape',windowsVirtualKeyCode:27});
  await send('Input.dispatchKeyEvent',{type:'keyUp',key:'Escape',windowsVirtualKeyCode:27});
  await send('Input.dispatchKeyEvent',{type:'keyDown',key:'F10',windowsVirtualKeyCode:121});
  await send('Input.dispatchKeyEvent',{type:'keyUp',key:'F10',windowsVirtualKeyCode:121});
  await wait(900);
  await measure('Full TV shelf movement', `window.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowDown',bubbles:true}));window.dispatchEvent(new KeyboardEvent('keyup',{key:'ArrowDown',bubbles:true}));`);
  writeFileSync('test-output/motion-0.6.0/animation-frames.json',JSON.stringify(frames,null,2)+'\n');
  assert.ok(frames.every(result=>result.maxMs<16.7),'animation frame times must remain under 16.7ms');
} finally {
  clearTimeout(timeout); ws.close();
}
