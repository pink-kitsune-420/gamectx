import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { GamectxClient } from '../bindings/typescript/gamectx.mjs';
const root = fileURLToPath(new URL('../', import.meta.url));
const client = new GamectxClient(root + 'examples/kit/project.json', root + 'target/debug/gamectx');
const snapshot = JSON.parse(fs.readFileSync(root + 'examples/kit/snapshot.json', 'utf8'));
try {
  const replies = await Promise.all([
    client.request('doctor', {}),
    client.request('build', {recipe:'conversation', snapshot}),
    client.request('query', {recipe:'conversation', snapshot, sql:'SELECT COUNT(*) FROM stock'}),
  ]);
  assert.deepEqual(replies[0].warnings, []);
  assert.equal(replies[1].inspection.receipt.total_bytes, 646);
  assert.equal(replies[2], 3);
  await assert.rejects(client.request('validate', {name:'inspect_item', arguments:{id:42}}));
  assert.equal((await client.request('validate', {name:'inspect_item', arguments:{id:'grove:Élan.9007199254740993'}})).arguments.id, 'grove:Élan.9007199254740993');
  await assert.rejects(client.request('query', {recipe:'conversation', snapshot, sql:'SELECT secret FROM stock'}));
  await assert.rejects(client.request('validate', {name:'inspect_item', arguments:{id:NaN}}));
  assert.deepEqual((await client.request('doctor', {})).warnings, []);
  console.log(JSON.stringify({node_bridge_checks:8, api_spend_usd:0}));
} finally { client.close(); }
