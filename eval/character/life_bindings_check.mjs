import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {GamectxClient,Character} from '../../bindings/typescript/gamectx.mjs';
const cases=JSON.parse(readFileSync('target/life-csharp/cases.json','utf8'));const client=new GamectxClient(null,'target/debug/gamectx');
try { for (const c of cases){const r=c.request;try {const got=await client.request(r.op,r.args);assert.notEqual(c.expected,'ERROR');assert.deepEqual(got,JSON.parse(c.expected).result);}catch(e){if(c.expected!=='ERROR')throw e;}}
 const setting=JSON.parse(readFileSync('examples/fixtures/characters/setting.json'));const def=JSON.parse(readFileSync('examples/fixtures/characters/merchant-life.json'));const npc=await Character.create(client,'helper','counter','merchant',['merchant','player']);await npc.lifeConfigure(setting,def,{enabled:true});assert.equal((await npc.lifeContext(['trade'],{facts:4,facets:2,attitudes:2,episodes:2,exchanges:1})).revision,0);await npc.close();
 console.log(JSON.stringify({fixtures:cases.length,model_calls:0,api_spend_usd:0}));
}finally{client.close();}
