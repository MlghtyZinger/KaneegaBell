// Render three original 1080p/60 fps KB intro videos with the owner's full recordings.
// No app assets/settings are replaced. Requires existing FFmpeg and Playwright.
import { readFileSync, writeFileSync, mkdirSync, existsSync, mkdtempSync, unlinkSync, rmdirSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createRequire } from 'node:module';
import { resolve, join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
const require = createRequire(import.meta.url);
const { chromium } = require(process.env.KB_PLAYWRIGHT ?? 'C:/Users/watch/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const args = process.argv.slice(2);
const nextBatch = args.includes('--next-batch');
const remaining = args.includes('--remaining') || nextBatch;
const sonyRevision = args.includes('--sony-revision');
const classic = args.includes('--classic') || remaining || sonyRevision;
const replace = args.includes('--replace');
const only = args.find(arg => arg.startsWith('--only='))?.slice('--only='.length);
const output = resolve(args.find(arg => !arg.startsWith('--')) ?? join(repo, 'docs/intro-video-previews', classic ? 'classic' : ''));
mkdirSync(output, { recursive: true });
const clips = sonyRevision ? [
  {scene:'ps1',key:'ps1',audio:'psx.mp3',edits:[[0,6.2],[12.25,16.05]],audioFadeOuts:[.12,.18],poster:7.1,title:'PS1 longer musical phrases with KB emblem',cues:[{event:'final emblem reveal',sourceTime:12.37,audioTime:6.32,visualTime:6.32}]},
  {scene:'ps2',key:'ps2',audio:'ps2-nostalgic-startup.mp3',edits:[[0,7.6],[14.4,16.4]],poster:8.3,title:'PS2 longer tower sequence with aligned chime',cues:[{event:'title midpoint',sourceTime:14.59,audioTime:7.79,visualTime:7.79}]},
  {scene:'psp',key:'psp',audio:['psp part 1.mp3','psp part 2.mp3'],poster:4.6,title:'PSP KB with Kaneegabell Computer Entertainment',cues:[{event:'logo midpoint and gold flare',sourcePart:2,sourceTime:.51,audioTime:3.932,visualTime:3.932}]},
].map(clip=>({...clip,fileTag:'revised',colors:['#d2462a','#5a140a']})) : remaining ? [
  {scene:'ps1',key:'ps1',audio:'psx.mp3',edits:[[0,6.2],[12.25,16.05]],audioFadeOuts:[.12,.18],poster:7.1,title:'PlayStation style with KB branding'},
  {scene:'ps2',key:'ps2',audio:'ps2-nostalgic-startup.mp3',edits:[[0,7.6],[14.4,16.4]],poster:3.5,title:'PS2 style with KB branding'},
  {scene:'psp',key:'psp',audio:['psp part 1.mp3','psp part 2.mp3'],poster:4.9,title:'PSP boot and game launch with KB branding'},
  {scene:'xbox360',key:'xbox360',audio:'xbox-360-startup.mp3',poster:5.8,title:'Xbox 360 sphere and KB emblem',cues:[{event:'ring and title flourish',audioTime:3.75,visualTime:3.75}]},
  {scene:'switch',key:'switch',audio:'switch-sound.mp3',holdDuration:1.8,poster:.6,title:'Switch snap and clear KB name',cues:[{event:'controller landing',audioTime:.06,visualTime:.06}]},
  {scene:'konami',key:'konami',audio:'konami.mp3',edits:[[0,5.25]],audioFadeOuts:[.18],poster:3.6,title:'Konami metallic ribbons with a single musical phrase',cues:[{event:'color resolve midpoint',audioTime:2.50,visualTime:2.50}]},
  {scene:'rockstar',key:'rockstar',audio:'rdr2_shotgun_blast.mp3',poster:3.2,title:'Rockstar engraved shells and blast reveal',cues:[{event:'shotgun flash',audioTime:5.08,visualTime:5.08}]},
].map(clip=>({...clip,fileTag:'classic',colors:['#d2462a','#5a140a']})) : classic ? [
  { scene:'gba', fileTag:'classic', audio:'game-boy-advance-sp-intro.mp3', key:'gba', title:'GBA style with KB branding', poster:3.35, colors:['#343383','#343383'] },
  { scene:'gamecube', fileTag:'classic', audio:'gamecube intro.mp3', key:'gamecube', title:'GameCube style with KB branding', poster:5.85, colors:['#a99ce8','#a99ce8'] },
  { scene:'ps3', fileTag:'classic', audio:'ps3boot.mp3', key:'ps3', title:'PS3 style with KB branding', poster:4.85, colors:['#f5f5f5','#f5f5f5'] },
] : [
  { scene:'prism', audio:'game-boy-advance-sp-intro.mp3', key:'gba', title:'Prism', poster:3.35, colors:['#7564bc','#353267'] },
  { scene:'orbit', audio:'gamecube intro.mp3', key:'gamecube', title:'Orbit', poster:5.85, colors:['#8c75d9','#281957'] },
  { scene:'aurora', audio:'ps3boot.mp3', key:'ps3', title:'Aurora', poster:4.7, colors:['#5795b9','#112b4c'] },
];
const run = (tool,args) => {
  const result = spawnSync(tool,args,{maxBuffer:32*1024*1024,windowsHide:true});
  if(result.status!==0)throw new Error(`${tool}: ${result.stderr?.toString()}`);
  return result.stdout;
};
const browser = await chromium.launch({headless:true,executablePath:process.env.KB_BROWSER ?? 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe'});
const reports = [];
let joinedAudioDir;
try {
  const page=await browser.newPage({viewport:{width:1920,height:1080},deviceScaleFactor:1});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.setContent('<canvas aria-label="KB intro rendering"></canvas>');
  await page.addScriptTag({content:readFileSync(join(repo,'tools/sounds',classic ? 'classic-intro-scenes.js' : 'intro-video-scenes.js'),'utf8')});
  // Load a supplied local typeface in this page only; never install or bundle it.
  for (const [name,path] of classic ? [
    ['KBPS3Startup',process.env.KB_INTRO_FONT],
    ['KBConsoleWordmark',process.env.KB_WORDMARK_FONT],
    ['KBPortableMark',process.env.KB_MONOGRAM_FONT],
  ] : []) {
    if (!path) continue;
    const font = readFileSync(path).toString('base64');
    await page.evaluate(async ({data,name}) => {
      const face = new FontFace(name, `url(data:font/otf;base64,${data})`);
      document.fonts.add(await face.load());
      if (!document.fonts.check(`40px ${name}`)) throw new Error(name+' failed to load');
    }, {data:font,name});
  }
  const master=readFileSync(join(repo,'src-tauri/icons/icon-master.svg'),'utf8');
  for(const clip of nextBatch ? clips.filter(clip=>!['ps1','ps2','psp'].includes(clip.key)) : clips){
    if (only && clip.key !== only) continue;
    const stem=`kb-${clip.key}-${clip.fileTag ?? clip.scene}`;
    const target=join(output,`${stem}.mp4`);
    if(existsSync(target)&&!replace)throw new Error('Refusing to replace existing preview without --replace: '+target);
    const sources = (Array.isArray(clip.audio) ? clip.audio : [clip.audio]).map(file=>join(repo,'audio files',file));
    let source = sources[0];
    if (sources.length > 1 || clip.edits) {
      joinedAudioDir = mkdtempSync(join(tmpdir(),'kb-intro-audio-'));
      source = join(joinedAudioDir,'joined.wav');
      const filter = clip.edits ? clip.edits.map(([start,end],i)=>
        `[0:a]atrim=start=${start}:end=${end},asetpts=PTS-STARTPTS,afade=t=in:d=0.008,afade=t=out:st=${end-start-(clip.audioFadeOuts?.[i] ?? .025)}:d=${clip.audioFadeOuts?.[i] ?? .025}[a${i}]`
      ).join(';') + ';' + clip.edits.map((_,i)=>`[a${i}]`).join('') + `concat=n=${clip.edits.length}:v=0:a=1[out]`
        : '[0:a][1:a]concat=n=2:v=0:a=1[out]';
      run('ffmpeg',['-v','error',...sources.flatMap(file=>['-i',file]),'-filter_complex',filter,'-map','[out]','-ar','48000','-ac','2','-c:a','pcm_s16le',source]);
    }
    const pcm=run('ffmpeg',['-v','error','-i',source,'-ac','1','-ar','48000','-f','f32le','-']);
    const samples=new Float32Array(pcm.buffer,pcm.byteOffset,pcm.length/4);
    const recordingDuration=samples.length/48000, duration=clip.holdDuration ?? recordingDuration, fps=60, frames=Math.ceil(duration*fps);
    const energy=[];
    for(let i=0;i<samples.length;i+=480){let sum=0;for(let j=i;j<Math.min(samples.length,i+480);j++)sum+=samples[j]**2;energy.push(Math.sqrt(sum/480));}
    const peak=Math.max(...energy);
    const svg=master.replace('stop-color="#d2462a"',`stop-color="${clip.colors[0]}"`).replace('stop-color="#5a140a"',`stop-color="${clip.colors[1]}"`);
    await page.evaluate(svg=>window.prepareScene(svg),svg);
    const ffmpeg=spawn('ffmpeg',['-hide_banner','-loglevel','error',...(replace?['-y']:[]),'-f','image2pipe','-framerate',String(fps),'-vcodec','mjpeg','-i','pipe:0','-i',source,'-map','0:v:0','-map','1:a:0',...(clip.holdDuration?['-af','apad']:[]),'-c:v','libx264','-preset','fast','-crf','18','-threads','4','-pix_fmt','yuv420p','-c:a','aac','-b:a','192k','-ar','48000','-movflags','+faststart','-t',duration.toFixed(6),target],{windowsHide:true,stdio:['pipe','ignore','pipe']});
    let encodingError='';ffmpeg.stderr.on('data',data=>encodingError+=data.toString());
    const encoded=once(ffmpeg,'close');ffmpeg.stdin.on('error',()=>{});
    console.log(`Rendering ${clip.title}: ${duration.toFixed(3)} s, ${frames} frames`);
    for(let frame=0;frame<frames;frame++){
      const time=frame/fps, level=(energy[Math.min(energy.length-1,Math.floor(time*100))]??0)/peak;
      const jpg=await page.evaluate(({scene,time,level,duration})=>{window.drawScene(scene,time,level,duration);return document.querySelector('canvas').toDataURL('image/jpeg',.96).split(',')[1];},{scene:clip.scene,time,level,duration});
      if(!ffmpeg.stdin.write(Buffer.from(jpg,'base64')))await once(ffmpeg.stdin,'drain');
      if(frame>0&&frame%180===0)console.log(`${clip.title}: ${Math.round(frame/frames*100)}%`);
    }
    ffmpeg.stdin.end();
    const [code]=await encoded;if(code!==0)throw new Error(encodingError);
    const info=JSON.parse(run('ffprobe',['-v','error','-show_entries','stream=codec_type,codec_name,width,height,duration,r_frame_rate:format=duration,size','-of','json',target]).toString());
    const video=info.streams.find(s=>s.codec_type==='video'),audio=info.streams.find(s=>s.codec_type==='audio');
    assert.equal(video.width,1920);assert.equal(video.height,1080);assert.equal(video.r_frame_rate,'60/1');
    assert.equal(video.codec_name,'h264');assert.equal(audio.codec_name,'aac');
    assert.ok(Math.abs(Number(audio.duration)-duration)<.035,'Complete selected audio timeline');
    assert.ok(Math.abs(Number(video.duration)-Number(audio.duration))<.035,'A/V duration alignment');
    // Read back the encoded movie to make its poster, so review shows the actual result.
    run('ffmpeg',['-v','error',...(replace?['-y']:[]),'-ss',String(clip.poster),'-i',target,'-frames:v','1',join(output,`${stem}.png`)]);
    run('ffmpeg',['-v','error','-i',target,'-f','null','-']);
    reports.push({title:clip.title,file:target,audioSource:sources.length>1?sources:sources[0],audioEditRanges:clip.edits,audioFadeOuts:clip.audioFadeOuts,cueTimings:clip.cues,sourceDuration:recordingDuration,holdDuration:clip.holdDuration,videoDuration:Number(video.duration),audioDuration:Number(audio.duration),fps:60,width:1920,height:1080,bytes:Number(info.format.size),decodeCheck:'passed'});
    if (joinedAudioDir) { unlinkSync(source); rmdirSync(joinedAudioDir); joinedAudioDir = undefined; }
    console.log(`PASS ${clip.title}: selected audio timeline, A/V duration within one frame, clean decode`);
  }
  assert.deepEqual(errors,[]);
  writeFileSync(join(output,'verification.json'),JSON.stringify(reports,null,2)+'\n');
}finally{
  await browser.close();
  if (joinedAudioDir) { unlinkSync(join(joinedAudioDir,'joined.wav')); rmdirSync(joinedAudioDir); }
}
