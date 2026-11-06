import assert from 'node:assert/strict';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { GamectxClient } from '../bindings/typescript/gamectx.mjs';
const args = process.argv.slice(2);
assert.equal(args.length, 3, 'require --cap 0 FIXTURES');
assert.deepEqual(args.slice(0, 2), ['--cap', '0']);
const root = fileURLToPath(new URL('../', import.meta.url));
const cases = JSON.parse(fs.readFileSync(args[2] + '/cases.json', 'utf8'));
const client = new GamectxClient(root + 'examples/kit/project.json', root + 'target/debug/gamectx');
try {
  for (const entry of cases) {
    if (entry.expected === 'ERROR') {
      await assert.rejects(client.request('local', entry.request.args), undefined, entry.name);
    } else {
      const actual = await client.request('local', entry.request.args);
      assert.deepEqual(actual, JSON.parse(entry.expected).result, entry.name);
    }
  }
  console.log(JSON.stringify({information_node_bridge_checks:cases.length, api_spend_usd:0}));
} finally { client.close(); }
