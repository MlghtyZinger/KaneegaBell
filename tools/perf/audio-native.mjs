// Isolated release-WebView QA. Every write stays under a new system-temp profile.
// node tools/perf/audio-native.mjs <new-system-temp-directory> <developer-exe>
import { DatabaseSync } from 'node:sqlite';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { spawn } from 'node:child_process';
import { join, resolve, relative, isAbsolute } from 'node:path';
import { tmpdir } from 'node:os';
import assert from 'node:assert/strict';
const [output, exe] = process.argv.slice(2);
if (!output || !exe) throw new Error('usage: audio-native.mjs <new-system-temp-directory> <developer-exe>');
const inside = relative(resolve(tmpdir()), resolve(output));
if (!inside || inside.startsWith('..') || isAbsolute(inside) || existsSync(output)) throw new Error('Use a NEW directory strictly under system temp');
mkdirSync(join(output,'data'),{recursive:true});
const dbPath=join(output,'data','library.db');
const db=new DatabaseSync(dbPath);
const migrations=resolve('src-tauri/src/db/migrations');
const files=readdirSync(migrations).filter(f=>/^\d.*\.sql$/.test(f)).sort();
for(const file of files)db.exec(readFileSync(join(migrations,file),'utf8'));
db.exec(`PRAGMA user_version=${files.length}`);
const setting=db.prepare('INSERT OR REPLACE INTO settings (key,value) VALUES (?,?)');
for(const [key,value] of Object.entries({introDesktop:'off',introTv:'off',startInTv:false,soundDesktop:true,soundTv:true,volumeDesktop:15,volumeTv:15,consoleFeedbackReady:true,autoUpdateCheck:false,autoArt:false,autoMeta:false,lbEnabled:false,wdEnabled:false,railMode:'open'}))setting.run(key,JSON.stringify(value));
db.close();
const port=9437;
const proc=spawn(resolve(exe),[],{windowsHide:true,stdio:'ignore',env:{...process.env,KB_DATA_DIR:join(output,'data'),WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS:`--remote-debugging-port=${port} --autoplay-policy=no-user-gesture-required`}});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
let socket;
const results=[];
try{
  let target;
  for(let i=0;i<100&&!target;i++){
    try{target=(await(await fetch(`http://127.0.0.1:${port}/json`)).json()).find(p=>p.type==='page'&&p.title==='KaneegaBell');}catch{}
    if(!target)await sleep(100);
  }
  if(!target)throw new Error('Isolated WebView did not start');
  socket=new WebSocket(target.webSocketDebuggerUrl);
  await new Promise((r,j)=>{socket.addEventListener('open',r,{once:true});socket.addEventListener('error',j,{once:true});});
  let seq=0;const pending=new Map();
  socket.addEventListener('message',({data})=>{const m=JSON.parse(data);if(pending.has(m.id)){const {resolve,reject}=pending.get(m.id);pending.delete(m.id);m.error?reject(new Error(JSON.stringify(m.error))):resolve(m.result);}});
  const send=(method,params={})=>new Promise((resolve,reject)=>{const id=++seq;pending.set(id,{resolve,reject});socket.send(JSON.stringify({id,method,params}));});
  const evaluate=async(expression)=>{const r=await send('Runtime.evaluate',{expression,returnByValue:true,awaitPromise:true});if(r.exceptionDetails)throw new Error(r.exceptionDetails.exception?.description??JSON.stringify(r.exceptionDetails));return r.result.value;};
  const wait=async(expression)=>{for(let i=0;i<150;i++){if(await evaluate(expression))return;await sleep(100);}const debug=await evaluate(`({intro:document.querySelector('.intro')?.className,wait:!!document.querySelector('.intro-wait'),sources:window.__qaSources,decodeErrors:window.__qaDecodeErrors,body:document.body.innerText.slice(0,250)})`);throw new Error('Timeout: '+expression+' '+JSON.stringify(debug));};
  const key=async(key,code)=>{await send('Input.dispatchKeyEvent',{type:'keyDown',key,windowsVirtualKeyCode:code,nativeVirtualKeyCode:code});await send('Input.dispatchKeyEvent',{type:'keyUp',key,windowsVirtualKeyCode:code,nativeVirtualKeyCode:code});};
  const set=(key,value)=>evaluate(`window.__TAURI_INTERNALS__.invoke('set_setting',${JSON.stringify({key,value})})`);
  await send('Page.enable');
  await wait(`!!document.querySelector('.library')&&!document.querySelector('.intro-wait')`);
  const seed=new DatabaseSync(dbPath);
  const insert=seed.prepare('INSERT INTO games (platform_id,title,sort_title,search_key,launch_path,added_at) VALUES (?,?,?,?,?,?)');
  const ids=['gba','gc','wii','ps1','ps2','ps3','psp','segacd','saturn','xbox','xbox360','snes','n64','nds','3ds','switch'];
  for(let i=0;i<80;i++){const title=`QA Game ${String(i+1).padStart(3,'0')}`;insert.run(ids[i%ids.length],title,title.toLowerCase(),title.toLowerCase(),join(output,`fake-${i}.rom`),1);}
  seed.close();
  await send('Page.addScriptToEvaluateOnNewDocument',{source:`window.__qaSources=[];window.__qaDecodeErrors=[];const old=AudioBufferSourceNode.prototype.start;AudioBufferSourceNode.prototype.start=function(...args){window.__qaSources.push({at:performance.now(),duration:this.buffer?.duration,state:this.context.state});return old.apply(this,args);};const decode=AudioContext.prototype.decodeAudioData;AudioContext.prototype.decodeAudioData=function(...args){return decode.apply(this,args).catch(error=>{window.__qaDecodeErrors.push(error.message);throw error;});};`});
  const durations={konami:10.4461,rockstar:11.077,gba:4.7601,gamecube:10.1239,ps1:21.8906,ps2:16.632,ps3:6.6874,psp:6.1649,xbox360:7.8629,switch:.828};
  for(const [name,duration] of Object.entries(durations)){
    await set('introDesktop',name);await send('Page.reload');
    await wait(`!!document.querySelector('.intro.on')&&window.__qaSources?.some(s=>s.duration>.8)&&document.querySelector('.intro').getAnimations({subtree:true}).some(a=>typeof a.startTime==='number')`);
    const timing=await evaluate(`(()=>{const source=window.__qaSources.find(s=>s.duration>.8);const start=Math.min(...document.querySelector('.intro').getAnimations({subtree:true}).map(a=>a.startTime).filter(t=>typeof t==='number'));return {...source,animationOffsetMs:start-source.at};})()`);
    assert.ok(Math.abs(timing.duration-duration)<.015,name+' complete asset');assert.equal(timing.state,'running');assert.ok(Math.abs(timing.animationOffsetMs)<70,name+' start alignment');
    results.push({test:`Native ${name} decoded duration and start`,...timing});
    await key('Enter',13);await wait(`!document.querySelector('.intro,.intro-wait')`);
    console.log(`PASS native intro ${name}`);
  }
  await set('introDesktop','off');await set('tvMenu','ps3');
  await send('Page.reload');await wait(`!!document.querySelector('.library')&&!document.querySelector('.intro,.intro-wait')`);
  await key('F10',121);await wait(`!!document.querySelector('.console-ps3')`);
  await wait(`(()=>{const m=document.querySelector('.console-menu-music');return m&&!m.paused&&m.currentTime>.1;})()`);
  const media=await evaluate(`(()=>{const m=document.querySelector('.console-menu-music');return {duration:m.duration,src:m.currentSrc,loop:m.loop,volume:m.volume};})()`);
  assert.match(media.src,/menu_ps3_theme/);assert.ok(Math.abs(media.duration-134.374)<.1);assert.ok(media.loop&&media.volume>0);
  await evaluate(`document.querySelector('.console-menu-music').currentTime=document.querySelector('.console-menu-music').duration-.1`);await sleep(600);
  assert.ok(await evaluate(`(()=>{const m=document.querySelector('.console-menu-music');return !m.paused&&m.currentTime<2;})()`));
  results.push({test:'Native PS3 music playback and real end looping',...media});
  await evaluate(`window.__qaMusic=document.querySelector('.console-menu-music')`);await key('F10',121);await wait(`!!document.querySelector('.library')`);
  assert.ok(await evaluate(`window.__qaMusic.paused&&!window.__qaMusic.hasAttribute('src')`));
  const makers=await evaluate(`Array.from(document.querySelectorAll('.rail-maker')).map(b=>b.title)`);
  assert.deepEqual(makers,['Nintendo','PlayStation','Sega','Microsoft']);
  await evaluate(`Array.from(document.querySelectorAll('.rail-maker')).find(b=>b.title==='PlayStation').click()`);
  assert.ok(await evaluate(`Array.from(document.querySelectorAll('.rail-maker')).find(b=>b.title==='PlayStation').getAttribute('aria-expanded')==='true'`));
  results.push({test:'Native four makers, folding and TV source cleanup',passed:true});
  for(const [width,height] of [[1280,720],[1920,1080],[3840,2160]]){
    await send('Emulation.setDeviceMetricsOverride',{width,height,deviceScaleFactor:1,mobile:false});await sleep(200);
    assert.ok(await evaluate(`Array.from(document.querySelectorAll('.rail-row')).every(b=>{const r=b.getBoundingClientRect();return r.left>=0&&r.right<=innerWidth;})`));
    const shot=await send('Page.captureScreenshot',{format:'jpeg',quality:75});writeFileSync(join(output,`sidebar-${width}.jpg`),Buffer.from(shot.data,'base64'));
    results.push({test:`Native sidebar ${width}x${height}`,passed:true});
  }
  await send('Emulation.clearDeviceMetricsOverride');
  writeFileSync(join(output,'report.json'),JSON.stringify(results,null,2)+'\n');
  console.log(`PASS ${results.length} native checks; profile ${output}`);
  await evaluate(`window.__TAURI_INTERNALS__.invoke('plugin:window|close',{label:'main'})`).catch(()=>{});
  await sleep(500);
}finally{
  socket?.close();
  if(proc.exitCode===null){
    // Only this script's isolated test process, never an owner's instance.
    const {execFileSync}=await import('node:child_process');
    execFileSync('powershell.exe',['-NoProfile','-Command',`$p=Get-Process -Id ${proc.pid} -ErrorAction SilentlyContinue; if($p){ $p.CloseMainWindow() | Out-Null }`],{windowsHide:true});
  }
}
