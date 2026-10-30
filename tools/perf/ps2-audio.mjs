// Exercise the real PS2 audio module with an isolated audio clock.
import {readFileSync} from 'node:fs';
import {stripTypeScriptTypes} from 'node:module';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const events=[],gains=[];let running=false,sound=true;
class AudioContext {
 currentTime=100;state='running';destination={};
 createGain(){const node={gain:{value:0},connect(){}};gains.push(node);return node;}
 createBufferSource(){return {connect(){},disconnect(){},start(at){events.push({at,duration:this.buffer.duration,loop:this.loop||false});},stop(){}};}
 async decodeAudioData(url){return {duration:{intro:12.8,loop:60,bridge:4}[url]};}
}
let source=readFileSync('src/fx/ps2Audio.ts','utf8').replace(/^import .*;\r?\n/gm,'').replace(/export /g,'');
source=stripTypeScriptTypes(source)+'\n globalThis.test={preparePs2Audio,startPs2Intro,endPs2Intro,syncPs2Audio};';
const context=vm.createContext({AudioContext,introUrl:'intro',loopUrl:'loop',bridgeUrl:'bridge',document:{hidden:false},fx:{volume:()=>.6,soundOn:()=>sound,tv:()=>true,get:()=> 'ps2'},session:{running:()=>running},fetch:async url=>({arrayBuffer:async()=>url})});
vm.runInContext(source,context);const audio=context.test;
await audio.preparePs2Audio();audio.syncPs2Audio(true);await Promise.resolve();
await audio.startPs2Intro();
assert.deepEqual(events.slice(-2),[{at:100,duration:12.8,loop:false},{at:112.8,duration:60,loop:true}]);
assert.equal(gains[0].gain.value,.33);
const count=events.length;audio.endPs2Intro(true);
assert.equal(events.length,count,'Natural video end must not restart or cut the audio');assert.equal(gains[0].gain.value,.33);
sound=false;audio.syncPs2Audio(true);assert.equal(gains[0].gain.value,0);
sound=true;running=true;audio.syncPs2Audio(true);assert.equal(gains[0].gain.value,0);
running=false;audio.syncPs2Audio(false);await Promise.resolve();
console.log('PASS: continuous 12.8s intro + scheduled 60s loop, same gain at video end, sound/game-running gates.');
