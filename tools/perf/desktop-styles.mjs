// Browser layout/interaction QA with generated games, never a personal library.
// node tools/perf/desktop-styles.mjs [Vite URL] [output directory]
import { createRequire } from 'node:module';
import { mkdirSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.KB_PLAYWRIGHT ?? 'C:/Users/watch/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const output = process.argv[3] ?? 'test-output/styles-0.6.0';
mkdirSync(output, { recursive: true });
const server = process.argv[2] ? null : await (await import('../../node_modules/vite/dist/node/index.js')).createServer({server:{host:'127.0.0.1',port:1421,strictPort:true},optimizeDeps:{noDiscovery:true,include:[]}});
if (server) await server.listen();
const browser = await chromium.launch({ headless: true, executablePath: 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe' });
const results = [];
try {
  const page = await browser.newPage({ viewport: { width: 1600, height: 900 } });
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(process.argv[2] ?? 'http://127.0.0.1:1421/?games=100&art=1');
  await page.waitForSelector('.vgrid-cell');
  await page.evaluate(async () => {
    const { fx } = await import('/src/state/fx.ts');
    fx.set('introDesktop', 'off'); fx.set('introTv', 'off');
    fx.set('animDesktop', 'full'); fx.set('soundDesktop', false);
  });
  if (await page.locator('.intro').count()) await page.keyboard.press('Escape');
  await page.waitForTimeout(400);
  const ids = ['forge', 'starlight', 'obsidian', 'aurora', 'ps3', 'wii', 'ps2'];
  assert.deepEqual(await page.evaluate(async () => (await (await import('/src/api/index.ts')).api.listThemes())[0].map(t=>t.id).sort()), [...ids].sort());
  for (const id of ids) {
    await page.evaluate(async id => {
      const { api } = await import('/src/api/index.ts');
      const { applyTheme } = await import('/src/state/theme.ts');
      applyTheme(await api.setTheme(id));
      await document.fonts.ready;
    }, id);
    await page.waitForTimeout(250);
    const result = await page.evaluate(() => {
      const grid=document.querySelector('.vgrid').getBoundingClientRect();
      const dock=document.querySelector('.desktop-wii-dock')?.getBoundingClientRect();
      return { style:document.documentElement.dataset.desktopStyle, background:getComputedStyle(document.querySelector('.library')).backgroundImage,
        titleFont:getComputedStyle(document.querySelector('.here-title')).fontFamily,
        gridFits:grid.width>0&&grid.height>0&&grid.right<=innerWidth+1&&grid.bottom<=innerHeight+1,
        dockFits:!dock||dock.bottom<=innerHeight+1,
        ambientAnimations:document.querySelector('.desktop-scenery').getAnimations({subtree:true}).length };
    });
    assert.equal(result.style,id);
    assert.equal(result.gridFits,true);
    assert.equal(result.dockFits,true);
    assert.equal(result.ambientAnimations > 0, !['wii','ps2','ps3'].includes(id));
    if (id === 'ps2') {
      assert.equal(await page.locator('.desktop-scenery canvas.ps2-ring').count(),1);
      const before = await page.locator('.ps2-ring').evaluate(el=>el.toDataURL());
      await page.waitForTimeout(300);
      assert.notEqual(await page.locator('.ps2-ring').evaluate(el=>el.toDataURL()),before,'original PS2 ring moves');
    }
    if (id === 'forge') { assert.equal(await page.locator('.forge-flames').count(),0); assert.equal(await page.locator('.forge-embers i').count(),22); }
    if (id === 'starlight') {
      assert.equal(await page.locator('.star-field i').count(),90);
      assert.equal(await page.locator('.star-comets i').count(),0);
    }
    if (id === 'ps3') assert.equal(await page.locator('.xmb-background').count(),1);
    await page.screenshot({ path:join(output,`${id}-1600x900.png`) });
    results.push(result);
  }
  for (const id of ['ps3','wii','ps2']) {
    await page.evaluate(async id => { const { api }=await import('/src/api/index.ts');(await import('/src/state/theme.ts')).applyTheme(await api.setTheme(id)); },id);
    for (const [width,height] of [[960,540],[3840,2160]]) {
      await page.setViewportSize({width,height});
      await page.waitForTimeout(200);
      assert.ok(await page.evaluate(()=>{const r=document.querySelector('.vgrid').getBoundingClientRect();return r.width>0&&r.height>0&&r.bottom<=innerHeight+1}));
      await page.screenshot({path:join(output,`${id}-${width}x${height}.png`)});
    }
  }
  await page.setViewportSize({width:1600,height:900});
  await page.locator('.vgrid-cell').first().dblclick();
  await page.waitForSelector('.info-panel');
  assert.equal(await page.locator('.dp-cover').evaluateAll(els=>els.some(el=>el.getAnimations().length>0)),false,'cover has no travel animation');
  await page.keyboard.press('Escape');
  await page.keyboard.press('Escape');
  await page.waitForSelector('.settings');
  await page.locator('#set-style button.picker').click();
  await page.getByRole('option',{name:'Wii',exact:true}).click();
  assert.equal(await page.evaluate(()=>document.documentElement.dataset.desktopStyle),'wii');
  assert.equal(await page.locator('#set-tvTheme').count(),0,'one style picker only');
  await page.getByRole('button',{name:'Done',exact:true}).click();
  await page.keyboard.press('F10');
  await page.waitForSelector('.tv');
  assert.equal(await page.evaluate(()=>document.documentElement.dataset.tvStyle),'kb','desktop console style does not replace the TV menu');
  for (const level of ['reduced','off']) {
    await page.evaluate(async level => (await import('/src/state/fx.ts')).fx.set('animTv',level),level);
    assert.equal(await page.locator('.desktop-scenery').count(),0);
  }
  await page.evaluate(async () => (await import('/src/state/fx.ts')).fx.set('animTv','full'));
  for (const id of ids) {
    await page.evaluate(async id => {
      const {api}=await import('/src/api/index.ts');
      (await import('/src/state/theme.ts')).applyTheme(await api.setTheme(id));
    },id);
    assert.equal(await page.locator('.desktop-scenery').count(),0,'TV has no desktop scenery');
    assert.equal(await page.locator('.tv .xmb-background,.tv .ps2-ring').count(),0,'KB TV has no console background layers');
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('.tv')).getPropertyValue('--accent').trim()),await page.evaluate(()=>getComputedStyle(document.documentElement).getPropertyValue('--accent').trim()),'shared UI palette');
    assert.ok(await page.locator('.tv-bg').count(),'original game art background retained');
  }
  await page.getByRole('button',{name:'Menu style: KB',exact:true}).click();
  assert.equal(await page.locator('.tv-theme-options').count(),0,'theme grid removed from TV menu');
  assert.equal(await page.locator('.tv-style-picker .tv-menu-item').count(),4,'four menu layouts retained');
  await page.locator('.tv-style-picker').getByRole('button',{name:/^PS3 /}).click();
  await page.waitForSelector('.console-ps3 .xmb-background');
  const canvas=page.locator('.console-ps3 .xmb-background');
  const before=await canvas.evaluate(el=>el.toDataURL());
  await page.waitForTimeout(750);
  assert.notEqual(await canvas.evaluate(el=>el.toDataURL()),before,'XMB waves flow');
  await page.screenshot({path:join(output,'xmb-reference.png')});
  for (const level of ['reduced','off']) {
    await page.evaluate(async level=>(await import('/src/state/fx.ts')).fx.set('animTv',level),level);
    const still=await canvas.evaluate(el=>el.toDataURL());
    await page.waitForTimeout(150);
    assert.equal(await canvas.evaluate(el=>el.toDataURL()),still,'XMB stops for '+level);
  }
  await page.clock.install({time:new Date(2026,9,30,23,59,58)});
  await page.evaluate(()=>window.dispatchEvent(new Event('focus')));
  const beforeMidnight=await canvas.getAttribute('data-hue');
  await page.clock.fastForward(2200);
  assert.notEqual(await canvas.getAttribute('data-hue'),beforeMidnight,'daily color updates at local midnight');
  await page.evaluate(async ()=>{ const {fx}=await import('/src/state/fx.ts');fx.set('animTv','full');fx.set('tvMenu','kb'); });
  await page.evaluate(async () => {
    const {api}=await import('/src/api/index.ts');
    (await import('/src/state/theme.ts')).applyTheme(await api.setTheme('wii'));
  });
  await page.keyboard.press('F10');
  await page.waitForSelector('.library');
  assert.equal(await page.evaluate(()=>document.documentElement.dataset.desktopStyle),'wii');
  assert.equal(await page.evaluate(()=>document.documentElement.dataset.tvStyle),undefined);
  assert.deepEqual(errors,[]);
  writeFileSync(join(output,'browser-report.json'),JSON.stringify(results,null,2)+'\n');
  console.log('PASS: seven styles, console layouts at 960p/4K widths, animated backgrounds, no cover travel, style picker and separate TV menu.');
} finally { await browser.close(); await server?.close(); }
