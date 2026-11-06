import assert from 'node:assert/strict';
import fs from 'node:fs';
import { GamectxClient, Character } from '../bindings/typescript/gamectx.mjs';
const root = new URL('../', import.meta.url).pathname;
const fixtures = JSON.parse(fs.readFileSync(root + 'target/character-csharp/cases.json', 'utf8'));
const client = new GamectxClient(null, root + 'target/debug/gamectx');
try {
  for (const {request,expected} of fixtures) {
    if (expected === 'ERROR') await assert.rejects(client.request(request.op, request.args));
    else assert.deepEqual(await client.request(request.op, request.args), JSON.parse(expected).result);
  }
  const character = await Character.create(client, 'helper', 'scope:helper', 'npc:merchant', ['npc:merchant','player:a']);
  await character.bind(fixtures.find(c => c.request.op === 'character_bind').request.args.view);
  await character.enable(['look_around']);
  assert.equal((await character.call('look_around', {detail:'brief'}, 7)).location.id, 'room:shop');
  await character.close();
  console.log(JSON.stringify({persistent_character_node_checks:fixtures.length,helpers:1,api_spend_usd:0}));
} finally {client.close();}
