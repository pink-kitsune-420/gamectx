# gamectx

Compile game data and tool schemas into small, budgeted context packets for AI-driven games.

A game that asks a language model to choose NPC actions sends the same things every turn:
action schemas, rules, inventories, recent events, and lookups into item or creature lists.
Sent as plain JSON, most of that is repeated keys and repeated schema fragments. `gamectx`
packs it without losing anything, trims rows and columns only when you ask, puts the parts
that never change first so providers can cache them, and drops the least important parts
when a turn would go over budget, with a receipt that says what was dropped.

It is a Rust library and a command-line tool with **zero third-party dependencies**.

> **Alpha.** The API, command-line options and packed formats may change between
> `0.1.0-alpha` releases. Pin an exact version, and see [CHANGELOG.md](CHANGELOG.md).

**Docs:** [practical guide](docs/GUIDE.md) · [formats](docs/FORMAT.md) ·
[conformance cases](conformance/README.md) · [model eval](eval/README.md) · [roadmap](docs/ROADMAP.md)

## Measured on real models

The same 53 questions about one game state (8 actions, 48 inventory items, 20 NPCs) were
asked with the data as plain compact JSON and packed by gamectx, at temperature 0, on three
models through an OpenAI-compatible API. Input tokens are what the provider billed.

| Format | Correct (159) | Avg input tokens | vs JSON |
|---|---|---|---|
| Plain compact JSON | 133 | 2,797 | |
| gamectx tables grouped by the filtered column | 140 | 1,735 | **−38%** |
| + actions as `signatures` | **141** | **1,517** | **−46%** |

Per model, gamectx with signatures scored at least as well as JSON: Llama 3.2 3B 49 vs 48,
Mistral Small 3.2 24B 45 vs 41, Llama 3.3 70B 47 vs 44 (of 53), and chose the right action
30 of 30 times. Lookups and schema rules were near-perfect in every format; the differences
came from filtering questions. The [eval script](eval/model_eval.py) reruns this for about
$0.31 with all three models, or roughly a quarter of that with its default two smaller ones.

What the measurements show:

- **Keep values as values.** A common compression trick, replacing repeated strings with
  indexes into a lookup list, saved no tokens (words like `common` are already one token)
  and cut accuracy from 102 to 74 of 108 on a 36-question set: even the 70B model answered
  `7` instead of looking up what `7` meant. gamectx always writes values in full.
- **Group by what your game filters on.** Grouping NPCs by faction made "which keepers are
  guards?" correct 18 of 18 times, against 14 for JSON. Grouping items by kind *and then*
  rarity made "every rare weapon" correct 12 of 12 times on the two smaller models, against
  7 with one level, while using 2–3% fewer tokens.
- **Describe actions as signatures, not JSON Schema.** `power: integer 1..5` instead of
  `{"type":"integer","minimum":1,"maximum":5}`, with arguments shared between actions
  described once, used 12–14% fewer tokens than packed JSON Schema at equal or better
  accuracy. Plain-text tables, by contrast, saved nothing over JSON tables: tokenizers
  already treat `","` as a single token.
- **Send only the rows that matter.** Keeping only table rows that match words of the
  question (`relevant_to`) used 26% fewer input tokens than the full tables, with identical
  accuracy on every question type (100/106 both, on the two smaller models). Sending no
  rows at all when nothing matches (`relevant_fallback: "none"`) saved another 59%, losing
  only questions about a table as a whole, which models got wrong most of the time anyway.
- **Recall memories instead of resending history.** For 10 questions about a 60-event
  history, sending the 8 events gamectx recalled instead of all 60 used 84% fewer input
  tokens with the same accuracy (20/20 both, on the two smaller models).
- **Let the game do the math.** Asked to add up numbers from the data, the two smaller models
  answered 0 of 16 questions correctly. Writing an expression for gamectx to compute instead
  got 8 of 16, and every remaining miss chose the wrong rows, which `where` and `derive` do
  exactly in code.
- **Let the model request rows and totals.** On 29 filtering and arithmetic questions per
  small model, direct answers scored 33/58; model-written SQL executed by gamectx scored
  53/58. Numeric filters improved from 0/6 to 6/6 and arithmetic from 1/16 to 12/16.
  Packing the `query` tool with the actions scored 52/58 in a separate run. It costs extra
  input and a tool round trip; these runs still sent the full data. See the
  [query measurements](eval/README.md#query-tool).
- **Do numeric filters in code, or ask for brief reasoning.** Questions like "items with
  count at least 15" were answered correctly 1 time in 27 across all formats. Asking the
  model to list candidate rows before its final answer raised filtering accuracy from
  37/54 to 46/54 for about 60 extra output tokens.

[docs/GUIDE.md](docs/GUIDE.md) turns these results into practical rules.

## What it does

From the included [Ember Keep example](examples/ember-keep) (8 actions, a 10-item inventory,
5 recent events, a 28-entry catalog):

```text
section              pack       priority stable   original     packed   saved
actions              signatures       10    yes       3182        997   68.7%
rules                raw               9    yes        142        142    0.0%
action_outline       outline           1    yes       3182        314   90.1%
inventory            table             6     no        831        444   46.6%
recent_events        table             7     no        318        227   28.6%
lights_nearby        catalog           3     no       1787        376   79.0%
mood                 raw               8     no         37         37    0.0%
all sections (bytes of compact JSON)                  9479       2537   73.2%

packet: 1906 bytes; budget 2000 bytes
  dropped action_outline (314 bytes, priority 1)
  dropped lights_nearby (376 bytes, priority 3)
```

- **Tables** (`table`, lossless): a list of objects with the same keys becomes one header
  plus value rows. With `group_by`, rows are grouped under one or more columns' values
  (nested), which is smaller still and makes filtering on those columns easier for models.
- **Views** (on tables): `where` filters exactly (`count >= 15`), `relevant_to` keeps the rows
  that match a player's message or the current event, `sort_by` + `limit` keep the top N
  (the 10 nearest NPCs), `fields` drops columns the decision does not need, and `round`
  trims long decimals.
- **Action signatures** (`signatures`, lossless): tool schemas as short text lines, with
  arguments shared between actions described once, such as `power: integer 1..5`. The
  recommended way to describe actions in a prompt.
- **Formulas** (`formula`): a safe expression language (no loops, no recursion, step limits)
  that the game or model defines once and the game evaluates exactly, as `derive` columns
  or through the model's `calc` and `define_formula` tools.
- **Queries** (`query`): bounded, read-only `SELECT` requests over tables the game exposes.
  The game filters, sorts, limits and aggregates; `query::tool()` describes the tool and
  `query::run()` executes the model's SQL. No database or SQL dependency required.
- **Memory** (`memory`): facts and events with tags, importance and turn, recalled by
  relevance instead of resending history, with a ready-made `remember` tool for the model.
- **Context** (library): register tools, prompts and constants once; add variables and
  tables each turn; build a packet within your budget.
- **Tool schemas** (`schema`, lossless): for tools signatures cannot express (nested objects,
  arrays). Definitions become rows, and parameter rules repeated across tools move into
  shared `$defs`.
- Both are for actions described *in the prompt* (as in the measurements above, or a planner
  that writes action scripts). Tools passed through a provider's native `tools` parameter
  must stay in that provider's own format.
- **Outlines** (`outline`, lossy): `cast_spell(spell=spark|salt_ward|mend|hush,target,power:1..5)`,
  a cheap reminder of what actions exist.
- **Catalogs** (`catalog`): search a registry and send one small page instead of the whole list.
  Each catalog has a `generation` fingerprint, so a follow-up page request is refused if
  the data changed in between.
- **Budgets**: stable sections first, then per-turn sections. Over budget, the lowest-priority
  optional section is dropped, and required sections fail loudly instead of being cut.
- **Token estimates**: never optimistic. Uncalibrated, one byte counts as one token. Feed it
  the input-token counts your provider reports and it learns a per-model ratio, used with a
  2x safety margin.
- **Verification**: `gamectx verify` unpacks every lossless section and checks it matches
  the original exactly, or for a table with a view, the rows the view selected. Numbers
  keep their original text (`1.50` stays `1.50`) unless a `round` view changes them.

## Quick start

```bash
cargo install --git https://github.com/pink-kitsune-420/gamectx --tag v0.1.0-alpha.2
# or, from a clone:
cargo build --release
./target/release/gamectx report examples/ember-keep/manifest.json
./target/release/gamectx pack examples/ember-keep/manifest.json --receipt
./target/release/gamectx verify examples/ember-keep/manifest.json
./target/release/gamectx search examples/ember-keep/catalog.json "lanterns" --count 3
```

A manifest lists the sections of one packet:

```json
{
  "budget_bytes": 2000,
  "sections": [
    {"name": "actions", "pack": "signatures", "file": "tools.json", "priority": 10, "stable": true, "required": true},
    {"name": "inventory", "pack": "table", "group_by": ["kind", "rarity"], "file": "inventory.json", "priority": 6},
    {"name": "nearby_npcs", "pack": "table", "file": "npcs.json", "sort_by": "distance", "limit": 10,
     "fields": ["name", "role", "mood"], "priority": 7},
    {"name": "lights_nearby", "pack": "catalog", "file": "catalog.json", "query": "#lights", "count": 4, "priority": 3},
    {"name": "mood", "pack": "raw", "value": {"warden": "wary"}, "priority": 8}
  ]
}
```

See [docs/FORMAT.md](docs/FORMAT.md) for every field and packed shape.

### Porting

The formats are defined by 139 shared [conformance cases](conformance/README.md), each an input
plus its exact expected output. A port in Java, JavaScript or anything else is
conformant when it passes all of them, so every project using gamectx can check itself
against the same files.

### Token calibration

```bash
# After a request whose body was 2640 bytes, the provider reported 700 input tokens:
gamectx calibrate cal.json --scope "my-provider/my-model" --bytes 2640 --tokens 700
gamectx report manifest.json --budget-tokens 1300 --calibration cal.json --scope "my-provider/my-model"
```

Only a hash of the scope name is stored.

## Use it in your code

Register what stays the same once, then set what changes each turn:

```rust
use gamectx::{obj, Context, Table};

// At startup. Stable sections are packed once and lead every packet.
let mut game = Context::new().budget_bytes(6000);
game.tools("actions", tools)?;     // signatures, or packed JSON Schema if a tool needs it
game.prompt("rules", "Ilse guards the keep. The game validates every action.")?;

// Every turn.
let mut turn = game.turn();
turn.var("mood", "wary")?;
turn.table("inventory", inventory, Table::new().group_by("kind").group_by("rarity").relevant_to(player_said))?;
turn.table("plenty", stock, Table::new().filter("count", ">=", 15).fields(&["id", "count"]))?;
turn.table("nearby_npcs", npcs, Table::new().sort_by("distance").limit(10).fields(&["name", "role"]))?;
turn.var("event", obj! {"actor" => "player", "did" => "lit_lantern", "where" => "hall"})?;
turn.memory("memories", &memory, player_said, 8)?;   // gamectx::memory::Memory

let packet = turn.build()?;        // packet.text, and packet.receipt of what was dropped
let system = packet.system_prompt("You are the game logic for Warden Ilse.");
```

`tools` and `inventory` are `gamectx::Value`s, from `gamectx::json::parse` on your JSON or
built with `obj!` and `vec![...]`. Numbers, strings, booleans, `Option` and `Vec` convert
automatically.

**Other languages** can pipe a manifest to the command-line tool. Python:

```python
import json, subprocess

manifest = {"sections": [
    {"name": "actions", "pack": "signatures", "value": tools, "stable": True, "required": True},
    {"name": "inventory", "pack": "table", "group_by": ["kind", "rarity"], "value": inventory},
    {"name": "mood", "value": "wary"},
]}
packet = subprocess.run(["gamectx", "pack", "-"], input=json.dumps(manifest),
                        capture_output=True, text=True, check=True).stdout
```

The lower-level modules (`table`, `signatures`, `view`, `budget`, ...) stay available when
you need full control; see the [API docs](https://docs.rs/gamectx) once published, or
`cargo doc --open`.

### Give the model a query tool

Include `gamectx::query::tool()` with your actions, then execute its `sql` argument over
the tables the game allows it to read. Return the JSON result, or the error so the model
can correct its request. Table names must match those shown in the packet.

```rust
use gamectx::{json, query};

let rows = json::parse(r#"[{"id":"moss_cake","count":2,"weight":0.25},
    {"id":"ember_cake","count":6,"weight":3.75}]"#, 4096)?;
let tables = [("inventory", rows.as_array().unwrap())];
let answer = query::run(
    "SELECT SUM(count * weight) FROM inventory WHERE count >= 6", &tables)?;
assert_eq!(answer.to_json(), "22.5");
```

The CLI accepts an object of named tables and prints JSON:

```bash
gamectx query tables.json "SELECT id FROM inventory WHERE count >= 6 ORDER BY weight DESC LIMIT 3"
gamectx query --tool
```

This is a [small SQL subset](docs/FORMAT.md#queries), with `ORDER BY` and `LIMIT` applied
**before** aggregates. Arithmetic uses 64-bit floats; round when needed. The game controls
which tables are exposed, and query results still need to fit the next packet's budget.

## Be honest with your numbers

- **Bytes saved are not tokens saved.** In the run above, gamectx cut bytes by 58% but
  tokens by 46%. Measure with your provider's reported usage, which is what `calibrate`
  is for.
- **Results depend on your data and models.** The numbers above are one game state and
  three models. Test the decisions your game actually depends on, with the eval script
  or your own.
- **Exact counts need the provider's tokenizer.** gamectx estimates conservatively instead
  of pretending to know.
- **The model only proposes.** Packed context describes what exists. Your game still has to
  validate every action the model asks for.

## Related work

The header-plus-rows idea is common. [TOON](https://github.com/toon-format/toon) is a full
compact notation for LLM input, and several tools pack prompts by priority or compress tool
schemas for coding agents. gamectx focuses on what game loops need: lossless round-trips,
validation-first action schemas, paged registries with change detection, cache-friendly
ordering, and calibrated budgets, in one dependency-free tool.

## Contributing

Bug reports, questions and measurements from your own game are welcome as GitHub issues.
For code changes:

- **No new dependencies.** gamectx uses only Rust's standard library, so anyone can embed it
  without license or supply-chain review.
- **The conformance cases define the formats.** `cargo test` runs all of them. A change to
  any `conformance/**/expected.json` is a format change and needs a CHANGELOG entry
  labelled **[format]**.
- **Claims need numbers.** If a change is meant to save tokens or improve accuracy, include
  results from the eval script or your own measurements.

## License

MIT. See [LICENSE](LICENSE).
