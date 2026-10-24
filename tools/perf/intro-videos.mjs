// Check the packaged clips against the reviewed exports, then exercise the real player
// in the browser-only generated library. No owner library or installed app is opened.
import { readFileSync, writeFileSync, mkdirSync, existsSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { createRequire } from 'node:module';
import assert from 'node:assert/strict';
const approved = JSON.parse(readFileSync('docs/intro-videos-0.3.16.json')).clips;
const sources = Object.fromEntries(approved.map(r=>[r.key,r.preview]));
const output = process.argv[2];
if (!output) throw Error('Usage: node tools/perf/intro-videos.mjs <output-folder> [Vite URL]');
mkdirSync(output, { recursive: true });
const reports = [];
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
const run = (cmd,args) => {
  const result=spawnSync(cmd,args,{windowsHide:true,maxBuffer:16*1024*1024});
  assert.equal(result.status,0,result.stderr?.toString());return result.stdout;
};
for(const [key,preview] of Object.entries(sources)) {
  const asset=`src/assets/intros/${key==='rockstar'?'rdr1':key}.mp4`;
  const source=`docs/intro-video-previews/${preview}`;
  assert.equal(hash(asset),approved.find(r=>r.key===key).sha256,`${key}: approved asset hash`);
  if (existsSync(source)) assert.equal(hash(asset),hash(source),`${key}: packaged clip differs from approved export`);
  const info=JSON.parse(run('ffprobe',['-v','error','-show_entries','stream=codec_type,codec_name,width,height,r_frame_rate,duration,start_time:format=duration','-of','json',asset]));
  const video=info.streams.find(s=>s.codec_type==='video'),audio=info.streams.find(s=>s.codec_type==='audio');
  assert.equal(video.codec_name,'h264');assert.equal(video.width,1920);assert.equal(video.height,1080);assert.equal(video.r_frame_rate,'60/1');
  assert.ok(['aac','opus'].includes(audio.codec_name));
  assert.ok(Math.abs(+video.duration- +audio.duration)<1/60,`${key}: audio/video duration mismatch`);
  assert.ok(Math.abs(+video.start_time- +audio.start_time)<.025,`${key}: media start offset`);
  run('ffmpeg',['-v','error','-i',asset,'-f','null','-']);
  reports.push({key,asset,preview,sha256:hash(asset),videoDuration:+video.duration,audioDuration:+audio.duration,decode:'passed'});
  console.log(`PASS ${key}: exact reviewed export, full decode, A/V lengths within one frame`);
}
writeFileSync(`${output}/assets.json`,JSON.stringify(reports,null,2)+'\n');
const require=createRequire(import.meta.url);
const {chromium}=require(process.env.KB_PLAYWRIGHT??'C:/Users/watch/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const browser=await chromium.launch({headless:true,executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',args:['--autoplay-policy=no-user-gesture-required']});
try {
  const page=await browser.newPage({viewport:{width:1920,height:1080}});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>{
    window.__media=[];window.__sources=[];
    const start=AudioBufferSourceNode.prototype.start;
    AudioBufferSourceNode.prototype.start=function(...args){window.__sources.push({at:performance.now(),duration:this.buffer?.duration});return start.apply(this,args);};
    const play=HTMLMediaElement.prototype.play;
    HTMLMediaElement.prototype.play=function(...args){
      if(this.matches('.intro-video')&&!this.__record){
        const ctx=window.__qaAudio??=new AudioContext();void ctx.resume();
        const analyser=ctx.createAnalyser();analyser.fftSize=1024;
        const gain=ctx.createGain();gain.gain.value=0;
        ctx.createMediaElementSource(this).connect(analyser);analyser.connect(gain);gain.connect(ctx.destination);
        const record={src:this.src,duration:0,peak:0,playingAt:0,ended:false,volume:this.volume,muted:this.muted};
        this.__record=record;window.__media.push(record);
        const samples=new Float32Array(analyser.fftSize);
        const timer=setInterval(()=>{analyser.getFloatTimeDomainData(samples);record.peak=Math.max(record.peak,...samples.map(Math.abs));},10);
        this.addEventListener('playing',()=>{record.playingAt||=performance.now();record.duration=this.duration;});
        this.addEventListener('ended',()=>{record.ended=true;record.endedAt=performance.now();clearInterval(timer);});
        this.addEventListener('pause',()=>{record.paused=true;if(!this.ended)clearInterval(timer);});
      }
      return play.apply(this,args);
    };
  });
  await page.goto(process.argv[3]??'http://127.0.0.1:1427/?games=20');
  await page.waitForFunction(()=>!!window.__pad);
  await page.evaluate(()=>window.__pad('select'));
  await page.waitForFunction(()=>!document.querySelector('.intro,.intro-wait'));
  await page.evaluate(async()=>{
    window.__fx=(await import('/src/state/fx.ts')).fx;
    window.__play=(await import('/src/fx/Intro.tsx')).playIntro;
    window.__fx.set('soundDesktop',true);window.__fx.set('volumeDesktop',35);
    const invoke=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=(cmd,args)=>cmd==='intro_sound'?Promise.resolve(window.__override??new ArrayBuffer(0)):invoke(cmd,args);
    window.__media=[];
  });
  const currentKeys=await page.evaluate(async()=>Object.keys((await import('/src/state/fx.ts')).INTROS));
  assert.deepEqual(currentKeys,Object.keys(sources));
  for(const key of Object.keys(sources)){
    await page.evaluate(key=>window.__play(key),key);
    await page.waitForFunction(()=>document.querySelector('.intro-video')?.currentTime>.05);
    await page.waitForFunction(()=>window.__media.at(-1)?.ended,{},{timeout:15000});
    const result=await page.evaluate(()=>window.__media.at(-1));
    assert.equal(result.muted,false);assert.equal(result.volume,.35);assert.ok(result.peak>.0001,`${key}: no decoded audio signal`);
    await page.waitForFunction(()=>!document.querySelector('.intro'));
    reports.find(r=>r.key===key).playback=result;
    console.log(`PASS ${key}: actual app player completed with audio signal and correct volume`);
  }
  await page.evaluate(()=>window.__play('ps2'));
  await page.waitForFunction(()=>document.querySelector('video.intro-video')?.currentTime>.15);
  const old=await page.locator('video.intro-video').elementHandle();
  await page.evaluate(()=>window.__pad('select'));
  assert.equal(await old.evaluate(v=>v.paused),true);
  await page.waitForFunction(()=>!document.querySelector('.intro'));
  await page.evaluate(()=>{window.__fx.set('soundDesktop',false);window.__play('psp');});
  await page.waitForFunction(()=>document.querySelector('video.intro-video')?.currentTime>.05);
  assert.equal(await page.locator('video.intro-video').evaluate(v=>v.muted),true);
  await page.evaluate(()=>window.__pad('select'));
  await page.waitForFunction(()=>!document.querySelector('.intro'));
  await page.evaluate(()=>{window.__fx.set('soundTv',true);window.__fx.set('volumeTv',25);window.__pad('toggleTv');});
  await page.waitForFunction(()=>window.__fx.tv());
  await page.evaluate(()=>window.__play('ps3'));
  await page.waitForFunction(()=>document.querySelector('video.intro-video')?.currentTime>.05);
  assert.equal(await page.locator('video.intro-video').evaluate(v=>v.volume),.25);
  await page.evaluate(()=>window.__pad('select'));
  await page.waitForFunction(()=>!document.querySelector('.intro'));
  await page.evaluate(()=>{
    window.__pad('toggleTv');window.__fx.set('soundDesktop',true);
    const rate=48000,n=rate*3,bytes=new ArrayBuffer(44+n*2),v=new DataView(bytes);
    const str=(o,s)=>[...s].forEach((c,i)=>v.setUint8(o+i,c.charCodeAt(0)));
    str(0,'RIFF');v.setUint32(4,36+n*2,true);str(8,'WAVE');str(12,'fmt ');v.setUint32(16,16,true);v.setUint16(20,1,true);v.setUint16(22,1,true);v.setUint32(24,rate,true);v.setUint32(28,rate*2,true);v.setUint16(32,2,true);v.setUint16(34,16,true);str(36,'data');v.setUint32(40,n*2,true);
    for(let i=0;i<n;i++)v.setInt16(44+i*2,Math.sin(i*2*Math.PI*440/rate)*2000,true);
    window.__override=bytes;window.__sources=[];window.__play('switch');
  });
  await page.waitForFunction(()=>document.querySelector('video.intro-video')?.currentTime>.05);
  assert.equal(await page.locator('video.intro-video').evaluate(v=>v.muted),true);
  await page.waitForFunction(()=>window.__media.at(-1)?.ended);
  assert.equal(await page.locator('.intro').count(),1,'long custom audio must finish before closing');
  const custom=await page.evaluate(()=>({media:window.__media.at(-1),audio:window.__sources.at(-1)}));
  assert.ok(Math.abs(custom.media.playingAt-custom.audio.at)<50,'override starts with video playback');
  await page.waitForFunction(()=>!document.querySelector('.intro'));
  await page.evaluate(()=>{window.__override=null;window.__play('ps1');});
  await page.waitForFunction(()=>document.querySelector('video.intro-video')?.currentTime>.05);
  const replaced=await page.locator('video.intro-video').elementHandle();
  await page.evaluate(()=>window.__play('ps2'));
  await page.waitForFunction(()=>document.querySelector('video.intro-video')?.src.includes('ps2.mp4'));
  assert.equal(await replaced.evaluate(v=>v.paused),true);
  await page.evaluate(()=>window.__pad('select'));
  await page.waitForFunction(()=>!document.querySelector('.intro,.intro-wait'));
  await page.evaluate(async()=>{
    const invoke=window.__TAURI_INTERNALS__.invoke;
    window.__TAURI_INTERNALS__.invoke=(cmd,args)=>cmd==='get_setting_values'?Promise.resolve({introDesktop:'xbox360',introTv:'xbox360'}):invoke(cmd,args);
    await window.__fx.load();
  });
  assert.equal(await page.evaluate(()=>window.__fx.get('introDesktop')),'random');
  assert.equal(await page.evaluate(()=>window.__fx.get('introTv')),'random');
  assert.deepEqual(errors,[]);
  writeFileSync(`${output}/playback.json`,JSON.stringify({clips:reports,skip:'passed',mute:'passed',tvVolume:'passed',customOverride:custom,replacement:'passed',pageErrors:errors},null,2)+'\n');
  console.log('PASS controller skip, mute, TV volume, custom audio alignment/long tail, replacement cleanup');
}finally{await browser.close();}
