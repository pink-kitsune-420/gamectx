import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {GamectxClient} from '../bindings/typescript/gamectx.mjs';
assert(process.argv.includes('--cap')&&process.argv[process.argv.indexOf('--cap')+1]==='0');
const fixtures=JSON.parse(readFileSync('target/studio-csharp/cases.json','utf8'));
const client=new GamectxClient(null,'target/debug/gamectx');
try {
  for(const f of fixtures) {
    let got,error;
    try {got=await client.request(f.request.op,f.request.args);}catch(e){error=e;}
    if(f.expected==='ERROR')assert(error,'Expected rejection');
    else {if(error)throw error;assert.deepEqual(got,JSON.parse(f.expected).result);}
  }
  console.log(JSON.stringify({fixtures:fixtures.length,actual_provider_calls:0}));
}finally{client.close();}
