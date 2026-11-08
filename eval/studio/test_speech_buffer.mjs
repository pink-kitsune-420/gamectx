// Real player handlers, deterministic media clock; no engines or audio device.
import assert from 'node:assert/strict';
import {VoicePlayer} from '../../examples/lantern-vault/web/voice.mjs';
assert(process.argv.includes('--cap')&&process.argv[process.argv.indexOf('--cap')+1]==='0');
const voice={voice_id:'merchant',pitch:1,rate:1};
function player(){
 const calls=[],views=[];let audio;
 const p=new VoicePlayer({post:async(op,args)=>{calls.push({op,...args});return{result:{queued:true}};},emit:v=>views.push(v),synth:{cancel(){},getVoices(){return[];}},audioFactory:()=>audio={currentTime:0,duration:4,play:async()=>{},pause(){this.paused=true;},removeAttribute(){},load(){}}});
 return{p,calls,views,get audio(){return audio;}};
}
const speech={id:1,text:'First sentence. Next sentence.'};
const chunk={id:1,audio:{audio:'UklGRg==',mime:'audio/wav',start:0,through:30,unit:'buffered',sample_rate:8000,boundaries:[{frame:16000,through:16},{frame:32000,through:30}]}};
assert.equal(speech.text.length,30);
const h=player();await h.p.start(speech,'local_tts_http',voice);await h.p.chunk(chunk);assert.equal(h.calls.filter(c=>c.op==='synthesize').length,1);
h.audio.currentTime=1;h.audio.ontimeupdate();await h.p.serial;assert.equal(h.p.current.through,0);assert(h.views.at(-1).audio_time===1);assert(!h.calls.some(c=>c.event==='played'));
h.audio.currentTime=2.5;h.audio.ontimeupdate();await h.p.serial;assert.equal(h.p.current.through,16);assert.equal(h.calls.at(-1).through,16);
const oldEnd=h.audio.onended,oldTime=h.audio.ontimeupdate;await h.p.interrupt();assert(h.audio.paused);await oldEnd();oldTime();await h.p.serial;assert.equal(h.calls.at(-1).event,'interrupted');assert.equal(h.calls.at(-1).heard,16);assert(!h.calls.some(c=>c.event==='finished'));
const f=player();await f.p.start(speech,'local_tts_http',voice);await f.p.chunk(chunk);await f.audio.onended();await f.p.serial;assert.equal(f.calls.filter(c=>c.op==='synthesize').length,1);assert.equal(f.calls.at(-1).event,'finished');assert.equal(f.calls.at(-1).heard,30);
// A media clock never invents a character position inside a buffered phrase.
const m=player();await m.p.start(speech,'local_tts_http',voice);await m.p.chunk({...chunk,audio:{...chunk.audio,boundaries:[{frame:32000,through:30}]}});m.audio.currentTime=3.99;m.audio.ontimeupdate();await m.p.interrupt();assert.equal(m.calls.at(-1).heard,0);
// Progress of a previous line cannot update a replacement turn.
const r=player();await r.p.start(speech,'local_tts_http',voice);await r.p.chunk(chunk);const stale=r.audio.ontimeupdate;await r.p.start({id:2,text:'Replacement.'},'off',voice);stale();assert.equal(r.p.current.through,0);
const sample=player();await sample.p.start(speech,'local_tts_http',voice);await sample.p.chunk(chunk);sample.audio.currentTime=2.5;await sample.p.interrupt();assert.equal(sample.calls.at(-1).heard,16);
const invalid=player();await invalid.p.start(speech,'local_tts_http',voice);await assert.rejects(invalid.p.chunk({...chunk,audio:{...chunk.audio,boundaries:[{frame:0,through:1000}]}}));assert.equal(invalid.audio,undefined);await invalid.p.interrupt();
console.log(JSON.stringify({buffered_player_workflows:7,synthesis_requests_per_completed_line:1,interrupted_confirmed_prefix:16,api_spend_usd:0,actual_provider_calls:0,limits:'Fake media clock on real player callbacks. Completed phrase checkpoints only; no phoneme alignment or physical heard-character claim.'}));
