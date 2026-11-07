import {readFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {GamectxClient} from '../bindings/typescript/gamectx.mjs';
assert(process.argv.includes('--cap')&&process.argv[process.argv.indexOf('--cap')+1]==='0');
const fixtures=JSON.parse(readFileSync('target/viewpoints-bindings/cases.json','utf8'));
const client=new GamectxClient(null,'target/debug/gamectx');
try {
 for(const f of fixtures){let got,error;try{got=await client.request(f.request.op,f.request.args);}catch(e){error=e;}if(f.error)assert(error,'expected rejection');else{if(error)throw error;assert.deepEqual(got,f.expected.result);}}
 console.log(JSON.stringify({node_fixtures:fixtures.length,actual_provider_calls:0,api_spend_usd:0}));
}finally{client.close();}
