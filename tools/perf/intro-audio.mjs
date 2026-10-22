// Browser-only synthetic library QA; never opens a developer/installed library.
// node tools/perf/intro-audio.mjs <output-directory> [Vite URL]
import { mkdirSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import assert from "node:assert/strict";
import { join } from "node:path";
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.KB_PLAYWRIGHT ?? "C:/Users/watch/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright");
const output = process.argv[2];
if (!output) throw new Error("usage: intro-audio.mjs <output-directory> [Vite URL]");
mkdirSync(output, { recursive: true });
const browser = await chromium.launch({ executablePath: process.env.KB_BROWSER ?? "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe", headless: true, args: ["--autoplay-policy=no-user-gesture-required"] });
const results = [], errors = [];
try {
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript(() => {
    window.__audioEvents = [];
    const start = AudioBufferSourceNode.prototype.start;
    const stop = AudioBufferSourceNode.prototype.stop;
    AudioBufferSourceNode.prototype.start = function (...args) {
      const record = { at: performance.now(), duration: this.buffer?.duration, state: this.context.state, stopped: null };
      window.__audioEvents.push(record);
      this.__qa = record;
      return start.apply(this, args);
    };
    AudioBufferSourceNode.prototype.stop = function (...args) {
      if (this.__qa) this.__qa.stopped = performance.now();
      return stop.apply(this, args);
    };
  });
  await page.goto(process.argv[3] ?? "http://127.0.0.1:1427/?games=60");
  await page.waitForFunction(() => !!window.__pad);
  await page.evaluate(() => window.__pad("select"));
  await page.waitForFunction(() => !document.querySelector('.intro,.intro-wait'));
  await page.evaluate(async () => {
    const { fx } = await import('/src/state/fx.ts');
    fx.set('introDesktop','off'); fx.set('introTv','off'); fx.set('soundDesktop',true);
    fx.set('soundTv',true); fx.set('volumeTv',60); fx.set('consoleFeedbackReady',true);
    window.__qaFx = fx;
    window.__playIntro = (await import('/src/fx/Intro.tsx')).playIntro;
    const original = window.__TAURI_INTERNALS__.invoke;
    window.__qaSettings = {};
    window.__TAURI_INTERNALS__.invoke = async (cmd, args) => {
      if (cmd === 'set_setting') { window.__qaSettings[args.key] = args.value; return; }
      if (cmd === 'get_setting' && args.key in window.__qaSettings) return window.__qaSettings[args.key];
      return original(cmd, args);
    };
  });
  const durations = { konami:10.4461, rockstar:11.077, gba:4.7601, gamecube:10.1239, ps1:21.8906, ps2:16.632, ps3:6.6874, psp:6.1649, xbox360:7.8629, switch:.828 };
  for (const [key, duration] of Object.entries(durations)) {
    await page.evaluate((key) => { window.__audioEvents = []; window.__playIntro(key); }, key);
    await page.waitForSelector('.intro.on');
    await page.waitForFunction(() => document.querySelector('.intro')?.getAnimations({subtree:true}).some(a => typeof a.startTime === 'number'));
    const result = await page.evaluate(() => {
      const event = window.__audioEvents.find((event) => event.duration > .8);
      window.__introEvent = event;
      const animations = document.querySelector('.intro').getAnimations({subtree:true});
      return { ...event, animationOffsetMs: Math.min(...animations.map((a) => a.startTime).filter((n) => typeof n === 'number')) - event.at };
    });
    assert.ok(Math.abs(result.duration - duration) < .015, `${key} complete recording: ${JSON.stringify(result)}`);
    assert.equal(result.state, 'running');
    assert.ok(Math.abs(result.animationOffsetMs) < 60, `${key} audio/CSS common start: ${result.animationOffsetMs}`);
    if (key === 'rockstar') {
      await page.evaluate(()=>{window.__shotAt=0;const observer=new MutationObserver(()=>{if(document.querySelectorAll('.bh.small').length===9){window.__shotAt=performance.now();observer.disconnect();}});observer.observe(document.querySelector('.intro'),{childList:true,subtree:true});});
      await page.waitForFunction(()=>window.__shotAt>0);
      const shot = await page.evaluate(() => ({ elapsed: window.__shotAt-window.__introEvent.at, pellets: document.querySelectorAll('.bh.small').length }));
      assert.equal(shot.pellets,9); assert.ok(Math.abs(shot.elapsed-5080)<150,JSON.stringify(shot));
      results.push({ test:'Shotgun cue',...shot });
    }
    if (key === 'ps1') {
      await page.waitForTimeout(5000);
      assert.equal(await page.locator('.intro.leave').count(),0);
      assert.ok(await page.locator('.intro .ps1b').evaluate((el) => Number(getComputedStyle(el).opacity)<.01));
      await page.screenshot({path:join(output,'ps1-first-phase.png')});
      await page.waitForTimeout(8400);
      assert.ok(await page.locator('.intro .ps1b').evaluate((el) => Number(getComputedStyle(el).opacity)>.99));
      await page.screenshot({path:join(output,'ps1-second-phase.png')});
      await page.waitForSelector('.intro', {state:'detached',timeout:14000});
      const elapsed = await page.evaluate(() => performance.now()-window.__introEvent.at);
      assert.ok(elapsed>=duration*1000);
      results.push({test:'Full PlayStation audio before natural exit',elapsed});
    } else if (key === 'switch') {
      const snap = await page.locator('.jc .jl').evaluate(el => ({delay:getComputedStyle(el).animationDelay,duration:getComputedStyle(el).animationDuration}));
      assert.deepEqual(snap,{delay:'0s',duration:'0.06s'});
      await page.waitForSelector('.intro', {state:'detached',timeout:4000});
    } else {
      await page.evaluate(() => window.__pad('select'));
      await page.waitForSelector('.intro', {state:'detached'});
      assert.ok(await page.evaluate(() => window.__introEvent.stopped!==null));
    }
    results.push({test:`${key}: decoded duration, common start and cleanup`,...result});
    console.log(`PASS intro ${key}`);
  }
  // Replacing an intro during asynchronous loading must not resurrect it.
  await page.evaluate(() => {
    const original=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=async(cmd,args)=>{if(cmd==='intro_sound'){await new Promise(r=>setTimeout(r,250));return null;}return original(cmd,args);};
    window.__audioEvents=[];window.__playIntro('gba');window.__pad('select');
  });
  await page.waitForTimeout(500);
  assert.equal(await page.locator('.intro,.intro-wait').count(),0);
  assert.equal(await page.evaluate(()=>window.__audioEvents.filter(e=>e.duration>.8).length),0);
  results.push({test:'Cancelled loading cannot start late audio',passed:true});

  // Controller-only maker folding keeps focus on the maker after rows change.
  await page.evaluate(()=>window.__pad('left'));
  for(let i=0;i<10 && await page.locator('.rail-row.focused').getAttribute('title')!=='Nintendo';i++) await page.evaluate(()=>window.__pad('down'));
  assert.equal(await page.locator('.rail-row.focused').getAttribute('title'),'Nintendo');
  await page.evaluate(()=>window.__pad('select'));
  assert.equal(await page.locator('.rail-row.focused').getAttribute('title'),'Nintendo');
  assert.equal(await page.locator('.rail-row.focused').getAttribute('aria-expanded'),'false');
  await page.evaluate(()=>window.__pad('down'));
  assert.equal(await page.locator('.rail-row.focused').getAttribute('title'),'PlayStation');
  await page.evaluate(()=>window.__pad('select'));
  assert.equal(await page.locator('.rail-row.focused').getAttribute('title'),'PlayStation');
  await page.evaluate(()=>window.__pad('down'));
  assert.equal(await page.locator('.rail-row.focused').getAttribute('title'),'PlayStation'); // PS1 tab
  assert.equal(await page.locator('.rail-row.focused').getAttribute('aria-expanded'),null);
  await page.evaluate(()=>window.__pad('select'));
  const chosen=await page.evaluate(async()=> (await import('/src/state/library.ts')).library.platform());
  assert.equal(chosen,'ps1');
  await page.locator('.rail').hover();
  await page.screenshot({path:join(output,'compact-folding-sidebar.png')});
  results.push({test:'Controller maker folding and platform selection',passed:true});
  const saved=await page.evaluate(()=>window.__qaSettings.openPlatformMakers);
  await page.evaluate(()=>window.__pad('toggleTv'));
  await page.waitForSelector('.tv-style-switch');
  await page.evaluate(()=>window.__qaFx.set('tvMenu','ps3'));
  await page.waitForSelector('.console-ps3');
  const music=page.locator('.console-menu-music');
  await page.waitForFunction(()=>{const m=document.querySelector('.console-menu-music');return m&&!m.paused&&m.currentTime>.1;});
  let media=await music.evaluate(m=>({src:m.currentSrc,duration:m.duration,volume:m.volume,loop:m.loop}));
  assert.match(media.src,/menu_ps3_theme/);assert.ok(Math.abs(media.duration-134.4)<.1);assert.ok(media.loop&&media.volume>0);
  await music.evaluate(m=>m.currentTime=m.duration-.1);
  await page.waitForTimeout(600);
  assert.ok(await music.evaluate(m=>!m.paused&&m.currentTime<2));
  results.push({test:'PS3 music plays and loops at actual end',...media});
  await page.evaluate(()=>window.__qaFx.set('soundTv',false));
  assert.ok(await music.evaluate(m=>m.paused));
  await page.evaluate(()=>{window.__qaFx.set('soundTv',true);window.__qaFx.set('volumeTv',20);});
  await page.waitForFunction(()=>!document.querySelector('.console-menu-music').paused);
  assert.ok(Math.abs(await music.evaluate(m=>m.volume)-.09)<.001);
  await page.evaluate(()=>window.__playIntro('switch'));
  await page.waitForSelector('.intro.on');
  assert.ok(await music.evaluate(m=>m.paused));
  await page.evaluate(()=>window.__pad('select'));
  await page.waitForSelector('.intro',{state:'detached'});
  await page.waitForFunction(()=>!document.querySelector('.console-menu-music').paused);
  await page.evaluate(()=>{Object.defineProperty(document,'hidden',{configurable:true,value:true});document.dispatchEvent(new Event('visibilitychange'));});
  assert.ok(await music.evaluate(m=>m.paused));
  await page.evaluate(()=>{delete document.hidden;document.dispatchEvent(new Event('visibilitychange'));});
  await page.waitForFunction(()=>!document.querySelector('.console-menu-music').paused);
  results.push({test:'PS3 music obeys sound, volume, intro and visibility',passed:true});
  await page.evaluate(()=>window.__qaFx.set('tvMenu','wii'));
  await page.waitForFunction(()=>{const m=document.querySelector('.console-menu-music');return !m.paused&&m.currentSrc.includes('menu_wii_theme')&&m.currentTime>.1;});
  assert.ok(Math.abs(await music.evaluate(m=>m.duration)-74.208)<.1);
  await page.evaluate(()=>window.__qaFx.set('tvMenu','ps3'));
  await page.waitForFunction(()=>{const m=document.querySelector('.console-menu-music');return !m.paused&&m.currentSrc.includes('menu_ps3_theme')&&m.currentTime>.1;});
  await page.evaluate(()=>{window.__oldMusic=document.querySelector('.console-menu-music');window.__pad('toggleTv');});
  await page.waitForSelector('.library');
  assert.ok(await page.evaluate(()=>window.__oldMusic.paused&&!window.__oldMusic.hasAttribute('src')));
  const expanded=await page.locator('.rail-maker[aria-expanded="true"]').evaluateAll(els=>els.map(e=>e.title));
  assert.deepEqual(expanded,saved.map(id=>({nintendo:'Nintendo',sony:'PlayStation',sega:'Sega',microsoft:'Microsoft',other:'Other'})[id]));
  results.push({test:'Menu source switch, TV cleanup and remembered maker state',passed:true});
  for(const [width,height] of [[1280,720],[1920,1080],[3840,2160]]){
    await page.setViewportSize({width,height});
    await page.locator('.rail').hover();
    const bounded=await page.locator('.rail-row').evaluateAll(els=>els.every(e=>{const r=e.getBoundingClientRect();return r.width>0&&r.right<=innerWidth;}));
    assert.ok(bounded);results.push({test:`Sidebar layout ${width}x${height}`,passed:true});
  }
  assert.deepEqual(errors,[]);
  writeFileSync(join(output,'report.json'),JSON.stringify({results,errors},null,2)+'\n');
  console.log(`PASS ${results.length} checks; no browser runtime errors`);
}finally{await browser.close();}
