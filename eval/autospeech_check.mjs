import assert from 'node:assert/strict';
import {AutoSpeech} from '../examples/dungeon/web/autospeech.mjs';
const a=new AutoSpeech(),s={generation:1,speech:{id:4,pending:true,delivery:{stage:'validated',done:false}},effective_backends:{tts:'local_tts_http'}};
let checks=0;const check=(wanted,options)=>{assert.equal(a.take(s,options),wanted);checks++;};
check(false,{enabled:true});s.speech.pending=false;check(false,{enabled:false});check(false,{enabled:true,busy:true});check(false,{enabled:true,host:false});s.effective_backends.tts='off';check(false,{enabled:true});s.effective_backends.tts='local_tts_http';check(true,{enabled:true});check(false,{enabled:true});s.speech.id++;s.speech.delivery.stage='queued';check(false,{enabled:true});s.speech.delivery.stage='validated';check(true,{enabled:true});s.speech.id++;s.speech.delivery.done=true;check(false,{enabled:true});s.generation++;s.speech.delivery.done=false;check(true,{enabled:true});
console.log(JSON.stringify({checks,provider_calls:0,api_spend_usd:0}));
