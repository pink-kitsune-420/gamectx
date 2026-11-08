import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {GamectxClient} from '../../bindings/typescript/gamectx.mjs';
const cases=JSON.parse(readFileSync('target/archetype-party-csharp/cases.json','utf8'));
const client=new GamectxClient(null,'target/debug/gamectx');
try {
  for(const c of cases){
    try {const got=await client.request(c.request.op,c.request.args);assert.notEqual(c.expected,'ERROR');assert.deepEqual(got,JSON.parse(c.expected).result);}
    catch(e){if(c.expected!=='ERROR')throw e;}
  }
  console.log(JSON.stringify({fixtures:cases.length,model_calls:0,api_spend_usd:0}));
} finally {client.close();}
