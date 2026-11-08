// Prototype first: current-turn wait state and exclusive browser audio ownership.
import assert from 'node:assert/strict';
assert(process.argv.includes('--cap')&&process.argv[process.argv.indexOf('--cap')+1]==='0');
import {replyWaiting as waiting,AudioOwner} from '../../examples/lantern-vault/web/context.mjs';
const s={generation:3,speech:{pending:false,delivery:{done:true,stage:'played'}}};
assert(!waiting(s));assert(waiting(s,1));assert(waiting(s,0,4));s.speech.pending=true;assert(!waiting(s));s.speech.delivery.done=false;assert(!waiting(s));s.speech.delivery.stage='validated';assert(waiting(s));
console.log(JSON.stringify({prototype_conditions:6,real_model_calls:0,api_spend_usd:0}));

let held=false;
const locks={async request(name,options,fn){if(held)return fn(null);held=true;try{return await fn({name});}finally{held=false;}}};
const a=new AudioOwner(locks,()=>true),b=new AudioOwner(locks,()=>true);
assert(await a.acquire());assert(!await b.acquire());assert(await a.acquire());
a.release();await new Promise(r=>setTimeout(r,0));assert(await b.acquire());b.release();await new Promise(r=>setTimeout(r,0));
const background=new AudioOwner(locks,()=>false);assert(!await background.acquire());
console.log(JSON.stringify({audio_owner_checks:5,real_model_calls:0,api_spend_usd:0}));
