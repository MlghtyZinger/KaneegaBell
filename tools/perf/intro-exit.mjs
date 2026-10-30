// Exercise the actual exit callback without opening the owner's application.
import {readFileSync} from 'node:fs';
import vm from 'node:vm';
import assert from 'node:assert/strict';
const source=readFileSync('src/fx/Intro.tsx','utf8');
const body=source.split('  const leave = () => {')[1]?.split('\n  const skip =')[0];
assert.ok(body,'Intro exit callback found');
for(const [intro,tv,style,expectBlack] of [['ps2',false,'ps2',true],['ps2',true,'kb',true],['ps2',true,'wii',true],['ps2',true,'ps3',true],['ps2',true,'ps2',false],['onepiece',false,'kb',false]]){
 let black=false,leaving=false,finished=false;const queue=[];
 const context=vm.createContext({disposed:false,syncedPs2:false,endPs2Intro(){},leaving:()=>leaving,blackOut:()=>black,video:{pause(){}},hush(){},key:()=>intro,fx:{tv:()=>tv,get:()=>style},setBlackOut:v=>black=v,setLeaving:v=>leaving=v,at:(ms,fn)=>queue.push({ms,fn}),finish:()=>finished=true});
 const leave=vm.runInContext('(()=>{'+body.trim().replace(/;$/,'')+')',context);
 leave();assert.equal(black,expectBlack);assert.equal(leaving,!expectBlack);
 assert.equal(queue[0].ms,expectBlack?350:300);
 leave();assert.equal(queue.length,1,'Repeated skips cannot restart the exit');
 if(expectBlack){queue.shift().fn();assert.equal(leaving,true);assert.equal(queue[0].ms,300);}
 queue.shift().fn();assert.equal(finished,true);
}
console.log('PASS: PS2 fades to black outside the PS2 TV menu; normal handoff inside it; repeated skips guarded.');
