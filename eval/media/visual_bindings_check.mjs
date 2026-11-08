import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {GamectxClient} from '../../bindings/typescript/gamectx.mjs';
const cases=JSON.parse(readFileSync('target/visual-csharp/cases.json','utf8'));
const client=new GamectxClient(null,'target/debug/gamectx');
try {
  for(const c of cases) {
    let got, error;
    try {got=await client.request(c.request.op,c.request.args);}
    catch(e) {error=e;}
    if(c.expected==='ERROR') assert.ok(error,'Expected rejection: '+c.request.op);
    else {if(error)throw error;assert.deepEqual(got,JSON.parse(c.expected).result);}
  }
  console.log(JSON.stringify({fixtures:cases.length,model_calls:0,api_spend_usd:0}));
} finally {client.close();}
