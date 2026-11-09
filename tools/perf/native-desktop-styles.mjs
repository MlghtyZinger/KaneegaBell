// Release-WebView checks on the isolated generated 5,000-game library.
// node tools/perf/native-desktop-styles.mjs <debug port> [output directory]
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
const port=process.argv[2]??'9438',output=process.argv[3]??'test-output/styles-0.6.0/native';
mkdirSync(output,{recursive:true});
const page=(await(await fetch(`http://127.0.0.1:${port}/json`)).json()).find(t=>t.type==='page');
const ws=new WebSocket(page.webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.onopen=resolve;ws.onerror=reject});
let id=0;const pending=new Map();
ws.onmessage=({data})=>{const m=JSON.parse(data);if(pending.has(m.id)){pending.get(m.id)(m);pending.delete(m.id)}};
const send=(method,params={})=>new Promise(resolve=>{const n=++id;pending.set(n,resolve);ws.send(JSON.stringify({id:n,method,params}))});
const evaluate=async expression=>{const r=await send('Runtime.evaluate',{expression,awaitPromise:true,returnByValue:true});if(r.result?.exceptionDetails)throw Error(r.result.exceptionDetails.exception?.description??'evaluation failed');return r.result.result.value};
const invoke=(command,args={})=>evaluate(`window.__TAURI_INTERNALS__.invoke(${JSON.stringify(command)},${JSON.stringify(args)})`);
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const press=async(key,code)=>{await send('Input.dispatchKeyEvent',{type:'keyDown',key,windowsVirtualKeyCode:code});await send('Input.dispatchKeyEvent',{type:'keyUp',key,windowsVirtualKeyCode:code})};
const results=[];
const timeout=setTimeout(()=>{console.error('native style check timed out');process.exit(1)},90000);
try{
  assert.equal((await invoke('get_library')).games.length,5000,'test library only');
  const list=await invoke('list_themes');
  assert.deepEqual(list[0].map(t=>t.id),['forge','starlight','obsidian','aurora','ps3','wii','ps2']);
  for(const [key,value] of Object.entries({startInTv:false,introDesktop:'off',introTv:'off',animDesktop:'full',consoleFeedbackReady:true,soundDesktop:true,volumeDesktop:10}))await invoke('set_setting',{key,value});
  await send('Page.reload');await sleep(1500);
  await evaluate(`window.__styleSounds=0;const original=AudioBufferSourceNode.prototype.start;AudioBufferSourceNode.prototype.start=function(...args){window.__styleSounds++;return original.apply(this,args)}`);
  for(const style of list[0]){
    await evaluate(`document.querySelector('.topbar button[title="Settings"]').click()`);await sleep(200);
    await evaluate(`document.querySelector('#set-style button.picker').click()`);await sleep(100);
    await evaluate(`Array.from(document.querySelectorAll('.picker-option')).find(e=>e.textContent.trim()===${JSON.stringify(style.name)}).click()`);await sleep(250);
    await evaluate(`Array.from(document.querySelectorAll('.settings .topbar button')).find(e=>e.textContent.trim()==='Done').click()`);
    await sleep(400);
    const before=await evaluate('window.__styleSounds');
    await press('ArrowRight',39);await sleep(200);
    const sounds=await evaluate('window.__styleSounds')-before;
    const measured=await evaluate(`(async()=>{
      const samples=[];let start=0,last=0;
      await new Promise(done=>{const step=t=>{if(!start){start=last=t;window.dispatchEvent(new KeyboardEvent('keydown',{key:'ArrowRight'}))}else{samples.push(t-last);last=t}if(t-start<1000)requestAnimationFrame(step);else done()};requestAnimationFrame(step)});
      return {style:document.documentElement.dataset.desktopStyle,maxMs:Math.max(...samples),frames:samples.length,ambient:document.querySelector('.desktop-scenery').getAnimations({subtree:true}).length,dock:!!document.querySelector('.desktop-wii-dock'),focusShadow:getComputedStyle(document.querySelector('.card.focused')).boxShadow};
    })()`);
    assert.equal(measured.style,style.id);assert.equal(measured.ambient>0,!['wii','ps2','ps3'].includes(style.id));assert.equal(measured.dock,style.id==='wii');
    assert.notEqual(measured.focusShadow,'none');
    if(['ps3','wii','ps2'].includes(style.id))assert.ok(sounds>0,`${style.name} move sound`);
    else assert.equal(sounds,0,`${style.name} stays silent`);
    const shot=await send('Page.captureScreenshot',{format:'png'});
    writeFileSync(join(output,`${style.id}.png`),Buffer.from(shot.result.data,'base64'));
    results.push({...measured,sounds});
    console.log(style.name,JSON.stringify({...measured,sounds}));
  }
  await evaluate(`document.querySelector('.vgrid-cell').dispatchEvent(new MouseEvent('dblclick',{bubbles:true}))`);await sleep(350);
  assert.equal(await evaluate(`document.querySelector('.dp-cover').getAnimations().length`),0);
  await press('Escape',27);
  await evaluate(`document.querySelector('.topbar button[title="Settings"]').click()`);await sleep(200);
  await evaluate(`document.querySelector('#set-style button.picker').click()`);await sleep(100);
  await evaluate(`Array.from(document.querySelectorAll('.picker-option')).find(e=>e.textContent.trim()==='Forge').click()`);await sleep(250);
  await evaluate(`Array.from(document.querySelectorAll('.settings .topbar button')).find(e=>e.textContent.trim()==='Done').click()`);
  writeFileSync(join(output,'report.json'),JSON.stringify(results,null,2)+'\n');
  assert.ok(results.every(r=>r.maxMs<16.7),'Full animation frames under 16.7ms');
  console.log('PASS: native styles, matching menu sounds, animated desktop backgrounds, focus, Wii dock, and no cover travel.');
}finally{clearTimeout(timeout);ws.close()}
