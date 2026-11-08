// Original delayed-command and poll-coalescing oracle, offline fake HTTP promises.
import assert from 'node:assert/strict';
import {makePost,coalescedRefresh} from '../../examples/lantern-vault/web/context.mjs';
assert(process.argv.includes('--cap')&&process.argv[process.argv.indexOf('--cap')+1]==='0');
let state={generation:5},requests=[];
let release;
const send=makePost(async(url,options)=>{requests.push({url,options});await new Promise(r=>release=r);return {ok:false,json:async()=>({ok:false,error:'Scene changed.',generation:6})};},()=>state);
const delayed=send('chat',{text:'buy oil'});state={generation:6};assert.equal(requests[0].options.headers['X-Studio-Generation'],'5');release();await assert.rejects(delayed,e=>e.generation===6);assert.equal(requests.length,1);
let receipts=0;const receipt=makePost(async(url,options)=>{receipts++;assert(!('X-Studio-Generation' in options.headers));return {ok:true,json:async()=>({ok:true,result:{}})};},()=>undefined);await receipt('delivery',{id:1,event:'interrupted'});assert.equal(receipts,1);
let sent=0;const guard=makePost(async()=>{sent++;},()=>({generation:Number.MAX_SAFE_INTEGER+1}));await assert.rejects(guard('act',{}));assert.equal(sent,0);
let loads=0,finish;state={generation:4};const refresh=coalescedRefresh(async()=>{loads++;if(loads===1){await new Promise(r=>finish=r);state={generation:5};}else state={generation:6};},()=>state);
const first=refresh();const after=refresh(6);await Promise.resolve();finish();await Promise.all([first,after]);assert.equal(loads,2);assert.equal(state.generation,6);
let resets=0;const reset=coalescedRefresh(async()=>{resets++;},()=>({generation:1}));await assert.rejects(reset(9));assert.equal(resets,2);
let failures=0;const recovery=coalescedRefresh(async()=>{if(failures++===0)throw Error('offline');},()=>({generation:2}));await assert.rejects(recovery());await recovery(2);assert.equal(failures,2);
console.log(JSON.stringify({context_lifecycle_checks:6,actual_provider_calls:0,api_spend_usd:0}));
