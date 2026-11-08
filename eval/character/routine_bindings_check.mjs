import assert from 'node:assert/strict';
import fs from 'node:fs';
import { GamectxClient, Character } from '../../bindings/typescript/gamectx.mjs';
const root = new URL('../../', import.meta.url).pathname;
const fixtures = JSON.parse(fs.readFileSync(root + 'target/routine-csharp/cases.json', 'utf8'));
const client = new GamectxClient(null, root + 'target/debug/gamectx');
try {
  for (const { request, expected } of fixtures) {
    if (expected === 'ERROR') await assert.rejects(client.request(request.op, request.args));
    else assert.deepEqual(await client.request(request.op, request.args), JSON.parse(expected).result);
  }
  const npc = await Character.create(client, 'helper', 'scope:a', 'merchant', ['merchant']);
  const setup = fixtures.find(c => c.request.op === 'character_routine_configure').request.args;
  await npc.routineConfigure(setup.calendar, setup.definition, setup.defaults);
  await npc.routineUpdate(fixtures.find(c => c.request.op === 'character_routine_update').request.args.frame);
  assert.equal((await npc.routineSave()).frame.revision, 1);
  await npc.close();
  console.log(JSON.stringify({ node_checks: fixtures.length, helpers: 1, api_spend_usd: 0 }));
} finally { client.close(); }
