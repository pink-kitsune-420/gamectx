// Offline browser lifecycle harness. Real event handlers, fake devices; no audio/model call.
import assert from 'node:assert/strict';
const cap=process.argv.indexOf('--cap');assert(cap>0&&process.argv[cap+1]==='0','Offline harness requires --cap 0');
import {VoicePlayer,scalarOffset,prefix} from '../examples/dungeon/web/voice.mjs';
for(const n of [undefined,NaN,-1,2.5,999])assert.equal(scalarOffset('A😀 lamp',n),0);
assert.equal(scalarOffset('A😀 lamp',3),2);assert.equal(scalarOffset('A😀 lamp',2),1);assert.equal(prefix('A😀 lamp',3),'A😀 ');
const calls=[],views=[],utterances=[];let cancelled=0;
class Utterance {constructor(text){this.text=text;}}
const synth={cancel(){cancelled++;},getVoices(){return [{name:'local',localService:true}];},speak(u){utterances.push(u);}};
const p=new VoicePlayer({post:async(op,args)=>{calls.push({op,...args});return {result:{queued:true}};},emit:v=>views.push(v),synth,Utterance});
const s={id:1,text:'A😀 lamp is ready. This tail stays unheard.'},v={voice_id:'local',pitch:1,rate:1};
await p.start(s,'browser_web_speech',v);utterances[0].onboundary({charIndex:8});await p.serial;
assert.equal(p.current.through,7);await p.interrupt();utterances[0].onend();await p.serial;
assert.deepEqual(calls.map(x=>x.event),['queued','played','interrupted']);assert.equal(calls.at(-1).through,7);assert(!calls.some(x=>x.event==='finished'));assert(views.at(-1).status.includes('unheard'));assert(cancelled>=2);
await p.start({id:2,text:'One complete sentence.'},'browser_web_speech',v);utterances[1].onend();await p.serial;assert.equal(calls.at(-1).event,'finished');assert.equal(calls.at(-1).through,22);
// The backend is still waiting: interrupt must stop immediately, before a pending receipt resolves.
let release;const blocked=new Promise(resolve=>{release=resolve;});const ordered=[];let speech;
const q=new VoicePlayer({post:async(op,a)=>{ordered.push(a.event);if(a.event==='played')await blocked;return {result:{queued:true}};},synth:{...synth,speak(u){speech=u;}},Utterance});
await q.start({id:3,text:'First word. Hidden tail.'},'browser_web_speech',v);speech.onboundary({charIndex:6});const ending=q.interrupt();assert(q.current.closed);assert.equal(cancelled>=3,true);release();await ending;assert.deepEqual(ordered,['queued','played','interrupted']);
// HTTP fallback resumes only the unheard suffix, and ignores stale chunk callbacks.
const r=new VoicePlayer({post:async()=>({result:{queued:false}}),synth,Utterance});await r.start({id:4,text:'One two three.'},'off',v);r.current.through=4;await r.nextChunk();assert.equal(utterances.at(-1).text,'two three.');await r.interrupt();

let stopped=false,ended;
const chunks=[];
const h=new VoicePlayer({post:async(op,a)=>{chunks.push({op,...a});return {result:{queued:true}};},synth,Utterance,audioFactory:()=>({play:async()=>{},pause(){stopped=true;},removeAttribute(){},load(){},set onended(fn){ended=fn;},set onerror(fn){}})});
await h.start({id:5,text:'One word. Unheard tail.'},'local_tts_http',v);
await h.chunk({id:5,audio:{audio:'UklGRg==',mime:'audio/wav',start:0,through:4}});
await h.interrupt();assert(stopped);await ended();assert(!chunks.some(x=>x.event==='finished'||x.event==='played'));assert.equal(chunks.at(-1).through,0);

console.log(JSON.stringify({browser_lifecycle_checks:5,actual_provider_calls:0,api_spend_usd:0,limits:'Fake browser event/device harness; physical playback and microphone accuracy unmeasured.'}));
// A browser refusing to start audio must reissue an unheard ticket, not end the tale.
const blockedReceipts=[];const denied=new VoicePlayer({post:async(op,a)=>{blockedReceipts.push({op,...a});return {result:{queued:true}};},synth,Utterance,audioFactory:()=>({play:async()=>{throw Error('blocked');},pause(){}})});
await denied.start({id:44,text:'An unheard ending.'},'local_tts_http',v);
await denied.chunk({id:44,audio:{audio:'UklGRg==',mime:'audio/wav',start:0,through:18}});
assert.equal(blockedReceipts.at(-1).event,'blocked');assert.equal(blockedReceipts.at(-1).heard,0);assert(!blockedReceipts.some(r=>r.event==='interrupted'||r.event==='finished'));
