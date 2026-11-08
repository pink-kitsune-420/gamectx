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
> `0.x` prereleases. Pin an exact version, and see [CHANGELOG.md](CHANGELOG.md).

**Docs:** [practical guide](docs/GUIDE.md) · [formats](docs/FORMAT.md) · [all docs](docs/README.md) ·
[examples](examples/README.md) · [conformance cases](conformance/README.md) · [model eval](eval/README.md) ·
[roadmap](docs/ROADMAP.md)

## Layout (0.4.0a)

The library is grouped into domain modules; the [crate docs](src/lib.rs) list every submodule.

| Module | Contents |
|---|---|
| `gamectx::data` | strict JSON, lossless tables, views, safe formulas, catalogs, model-written SQL queries |
| `gamectx::packet` | the `Context` builder, budgeted assembly, manifests, tool schemas/signatures, full-request accounting, token estimates, sync |
| `gamectx::knowledge` | memory, recall, typed facts, lore, informational notes, inventory, observations, evidence selection, location, world, time windows |
| `gamectx::character` | character tools, life, dialogue, routines, parties, archetypes, cast packs, living world, `roleplay` policies |
| `gamectx::runtime` | reaction brains, legal-choice decisions, scheduling, procedures, outcomes, replay, request tickets, spending |
| `gamectx::deploy` | deployment profiles, scene-driven model policy, configuration tuning, turn telemetry |
| `gamectx::media` | visual specs and asset registry, voice delivery, provider routing |
| `gamectx::kit` | registration, recipes, validated callbacks, inspection, JSON-lines/native bridges |

`Context`, `Table`, `Turn`, `Value`, `Error`, `Result` and `obj!` stay at the crate root.
Repository folders follow the same grouping: [`examples/`](examples/README.md) by domain plus
shared `fixtures/` and the `lantern-vault/` game, [`docs/`](docs/README.md) by purpose and
[`eval/`](eval/README.md) by topic. Bindings live in `bindings/`, format cases in `conformance/`.

The [dynamic character guide](docs/characters/LIFE.md) records the five implementation stages from
[the agent brief](docs/research/ROLEPLAY_AGENT.md). Features default off; host rules retain authority.

## Configurable roleplay engine, world definitions and saves (0.4.0a)

[`character::roleplay::engine`](docs/characters/ROLEPLAY_ENGINE.md) gives any game one
roleplay brain per AI character. It handles greetings that remember encounters, social
lines, remembered names and preferences, perception, stage actions and directions,
commands over the turn's legal choices, and a model hand-off for everything else. Every
stage can be switched on or off and tuned, and all wording comes from a line book you
can extend or replace. Locations can host [props](docs/characters/ROLEPLAY_ENGINE.md#props)
with descriptions and registered interactions.

A [character sheet](docs/characters/ROLEPLAY_ENGINE.md#character-sheet) adds persona,
services, goals and needs, relationships, factions, trust-gated secrets and opinions. A
shared [world codex](docs/characters/ROLEPLAY_ENGINE.md#world-codex) adds places with
spots, ambience and house rules, entrances with open/locked/hidden states, factions,
timed events and quests. [`kit::save`](docs/characters/ROLEPLAY_ENGINE.md#saving-game-data-and-saves)
packs the authored side into one game data file and runtime memory into one save file
with any number of named, forkable states. The [world model](docs/guides/WORLD.md) adds
a timeline (the game's own record of what happened and who knows it) and places: kinds
such as workshop, forest or castle, nesting, and a nearby-places query.

Anything the authored stages don't cover falls through to a compiled [model
request](docs/characters/ROLEPLAY_ENGINE.md#model-requests-and-replies). It carries the
character's persona, speech style and what they know, but never their locked secrets.
The model's raw reply goes back through `reply`, which validates it before anyone hears
it.

The engine only proposes; your game validates and executes.

- **Conformance:** 130 engine and 22 save-file cases.
- **Parity:** the Rust output is byte-identical to independent Python references on
  7,500 randomized engine cases and on 2,932 of 3,000 randomized save workflows. The
  other 68 are stricter Rust validation of nested lore/memory data, a documented
  difference.
- **Local models ($0):**
  - Llama 3.2 3B Instruct: 92.6% of replies accepted. Fact hits were 61.1% with the
    brief and 11.1% without it.
  - Stheno 8B: same fact hits, but only 74.1% accepted because it overruns the sentence
    limit. See [the evaluation](docs/characters/ROLEPLAY_ENGINE.md#model-evaluation).

## Optional local RPG, voice and art studio (unreleased v0.3.1a)

Run `cargo run --example dungeon -- --web --cap 0` and open the printed loopback URL.
The [Lantern Vault studio](docs/studio/STUDIO.md) combines text RPG/party controls, approved art
and repair, live character inspection, TTS/STT and receipt-based barge-in. Text, image,
TTS and STT backends switch independently; all paid requests share a durable spending cap.
Default play needs no model or provider account. The Rust host uses only `std`; optional
HTTPS providers use system curl. Browser voice support varies.

**32 Python/Rust workflows**, **85 HTTP checks** and **5 fake-browser scenarios** run with
zero real provider calls / $0 API spend. All **1,090** prior conformance cases remain
unchanged. [Editable provider profiles](examples/lantern-vault/web/providers.json) select actual
model IDs per tier; unconfigured profiles retain deterministic fallbacks.

## Optional visual specs and approved art (unreleased v0.3.1a)

[Visual compiler and registry](docs/guides/VISUAL.md) lock an editable art bible, map live Life
attitudes/emotions and active routine/initiative poses, and include only perceived scene
cues. Content keys reuse approved/pinned assets; pending work returns compatible approved
fallbacks. Host inspection, repair constraints, seed rotation and hand-edited pins invalidate
stale candidates. Python, Node and native/C# sessions share the host controls.
`VisualDirector` (0.4.0a) adds tiered art bibles, format profiles, host fact selectors and
saved approvals that are re-verified after a restore: 35 cases and 3,000 randomized
workflows byte-identical to the Python reference, $0.

Run `cargo run --example visual`; edit [the art bible](examples/fixtures/visual/chime-art.json).
**64 Python/Rust workflows**, including **13 malformed cases**, match with **0 model calls**
and **$0 API spend**. All **1,026** prior conformance cases remain unchanged. Generation is
external; this measures metadata behavior, not image quality or image-model performance.

## Optional dynamic character continuity (unreleased v0.3.1a)

[Life APIs](docs/characters/LIFE.md) add approved character definitions, source-attributed experiences,
bounded appraisals, parallel conversation topics, acknowledged partial speech, durable
checkpoints, archive backpressure, causal recall/forgetting and purposeful initiative.
Opinions, accepted promises and actual event tokens drive initiative; routine work makes
zero model calls. Existing character sessions carry Python, Node/TypeScript and native/C#
host controls. All features and model tools remain opt-in; immutable project ABI is unchanged.

Run `cargo run --example dungeon -- --life-demo --size 3` for an original merchant's
interrupted repair, remembered preference, corrected name, changed opinion and reunion
following save/load. `--size 70 --life-tools` demonstrates expanded context and explicit
tool selection without loading a model. Editable definitions/configuration are included.
The [minimal Rust example](examples/character/character-life.rs) runs as a test. New
`--dialogue natural-scaled` optionally applies actual dialogue recall/exchange hints and
removes distractor lore for unknown small-model answers; earlier modes retain their bytes.

Independent Python/Rust checks cover **221 workflows**, including a **1,000-event** trace;
**47 additive conformance cases** preserve all **842** earlier cases. The host benchmark
for 128 experiences plus 128 appraisals measured median **176,881,587 → 7,338,259 ns** and
**2,517,745 → 22,025 allocator calls**, with identical final state. In-memory structural
validation makes zero heap allocations; these are synthetic host measurements, not model
speed or accuracy. [Evidence, method and limits](docs/evidence/LIFE_EVAL.md).

A retained local speech diagnostic made **24 calls**, **$0 API**: all **8/8** replies had
valid shapes, yet both variants invented birthplace details and the character-context
variant invented weather and produced unfinished replies. Balanced free-form prose remains
experimental; no model correctness or cache-hit improvement is claimed. Source-selected
answers remain the default. Human social-quality evaluation is still needed.

## Modular archetypes and multi-party scenes

The opt-in [archetype and multi-party layer](docs/characters/ARCHETYPES.md) provides ambient (0B),
service, quest and companion loadouts. Compose modules through `BlueprintBuilder`, gate
lore/facets/affordances by listener attitudes, render confirmed hearing from each AI's
perspective, and reserve bounded responders and spotlight routes through the shared ledger.
Run `cargo run --example party` and edit [the six-participant scene](examples/fixtures/party/chime-counter.json).
Offline evidence covers 68 Python/Rust workflows and 68 additive cases, with zero model calls.
Provider token counting, authoritative execution and hearing stay with the host.

## Turn-aware short speech follow-up

[Speech compiler v2](docs/evidence/LIFE_SPEECH_V2_EVAL.md) gates creative policy by host profile and
known-state, binds facts to explicit subjects, and keeps the current request outside JSON.
Run `cargo run --example character-speech`; the life dungeon demo also uses it. A bounded
sentence helper identifies punctuation endings and suspected schema-wall cutoffs without
rewriting raw speech. Legacy outputs and v1 artifacts stay unchanged.

In eight fresh paired local turns per arm, terminal endings improve **5/8→8/8** and
suspected 240-character cutoffs **3/8→0/8**. V2 origin replies avoid invented birthplaces,
but weather and stock invention remain. This is an experimental diagnostic, not a grounding
guarantee: **48 local calls**, **$0 API**, four regressions and four new holdouts.

## Adversarial probes and measured limits (unreleased v0.3.1a)

[Fresh probes](docs/evidence/ADVERSARIAL_EVAL.md) found and fixed unintended conversation actions,
hidden time conflicts and invalid restores: 15/57→57/57 checks pass; 900 engine steps
match the independent rules. The installed local Qwen variant produced valid JSON on
32/32 source/time tests but only 25/32 correct answers. Shorter context was faster and
one answer worse. Optional typed time assessment keeps that classification in host code.
An opt-in `--count-cache 8` reuses verified counts for identical requests: 18→12 model
calls over nine turns, with the same answers. All new spend $0; 159 local calls recorded.
Default tool exposure, prior outputs and immutable native ABI remain unchanged.

## Optional routines and source time knowledge (unreleased v0.3.1a)

[Routine APIs](docs/guides/ROUTINES.md) add bounded usual-activity definitions, deterministic
per-character timelines, actual observation history and attributed aging time ranges.
All features default off; hosts choose shared defaults, character overrides, individual
tools, task exposure and live permissions. Plans never prove experiences. Movement,
sensors, calendars, completion and action execution stay in the engine. No model calls
are needed for planning. Run `cargo run --example routines` or the offline dungeon
`--routines-demo`; editable files and persistent Python/Node/C# bridges are included.
69 independent Python/Rust workflows agree; 21 additive cases preserve all 797 prior
outputs. [Explicit task evidence selection and timer hints](docs/guides/ROUTINES.md#optional-task-selection-and-responsive-wakeups)
add 54 offline workflows; a measured host update optimization avoids copying retained
history. These are deterministic checks and a synthetic CPU benchmark, with no new
model-accuracy or full-turn latency claim. Model/API spend $0.

## Actor perception and adaptive context (unreleased v0.3.1a)

[Character tools](docs/characters/CHARACTERS.md) provide optional `look_around`, location descriptions,
world-state lookup and a single scoped dispatcher for notes, inventory and conversation
helpers. The game supplies visibility and knowledge: NPCs do not automatically know the
global clock or outdoor weather. Scene snapshots and tool permissions are revision-bound.
Task-focused context can grow with measured capacity while small-model attention stays
explicitly limited. Rust, Python, Node and C# bridges support persistent character sessions.
The [new checks](docs/evidence/CONVERSATION_PATCH_EVAL.md) report byte parity and retain failed
speech experiments separately from deterministic routing improvements.

## Playable dungeon testbed (v0.3.0a)

Run `cargo run --example dungeon -- --seed 7` for an original six-room text game.
[Lantern Vault](examples/lantern-vault/README.md) has deterministic rules, seeded variations,
replay files and a persistent JSON interface for local agents. It exercises scene packets,
typed affordability queries, bounded decisions and current-state validation. Six scripted
routes escape; optional model runs report actual wins separately from legal actions.
Basic play needs no model or network. For human play with local AI characters, run
`cargo run --example dungeon -- --ai --size 3 --seed 7` with Python 3 and an installed
Ollama model. NPCs answer free-form questions and remember scoped player quotes; natural
language actions use bounded choices and targeted clarification. Local rules validate
world changes. [Size-and-task workflows](docs/guides/WORKFLOWS.md) cover editable 1B, 1.5B, 3B,
8B, 30B and 70B hints and the measured limits of the local 3B pilot. New API spend $0.

Shared [conversation APIs](docs/guides/CONVERSATION.md) now provide scoped quote memory,
validated replies, entity references and one-use action confirmation for other hosts.
The dungeon's merchant reports unaffordable purchases locally and completes affordable
ones after yes/no confirmation. Inventory counts/sums and catalog/memory tags already
use the existing query and metadata APIs; hosts retain inventory ownership.

[Reported dialogue regressions](docs/evidence/DUNGEON_FEEDBACK.md) now preserve 11 playthrough
sequences: transaction handling and name recall work, while free-form speech can still
invent player history or item purposes. The offline audit separates those outcomes.

## Authored world information (v0.3.0a)

Use `cargo run --example dungeon -- --ai --world examples/lantern-vault/world.json` to load
editable character backgrounds and world facts. Only relevant, authorized records reach
a character. Factual questions select a host-supplied source or an explicit unknown;
player quotes stay distinct from lore and current inventory. Open-ended dialogue remains
generated. `--dialogue legacy` selects the earlier control.

Optional [information tools](docs/guides/INFO.md) let a model remember and look up reported
ownership, locations and other claims with original evidence. A read-only inventory tool
returns game-confirmed owners/counts; unique instances and validated host transfers are
supported. Claims never change inventory. See [the runnable example](examples/knowledge/info_tools.rs).
Structured lore and `--dialogue natural` are available experimentally; local tests still
show factual hallucinations, so source selection remains the default.

Optional [conversation tools](docs/guides/INTERACTIONS.md) add witnessed events, promises,
relationship notes, clarification and recipient-bound offers. Explicit participant IDs
support multiplayer hosts; the dungeon remains a single-player example. Enable tools
selectively and keep execution in the game. [Runnable example](examples/character/conversation_tools.rs).

The reusable [lore and answer APIs](docs/guides/LORE.md) keep the Rust core dependency-free.
[Local tests](docs/evidence/LORE_EVAL.md) retain the failed pilots, 17/20 on twenty new questions,
20/20 on their tuned rerun, and 7/8 on eight additional questions. The last miss has a local
regression repair; these are small diagnostics, not universal accuracy. Every run $0 API.

## Further local optimizations (v0.3.0a)

[Optional optimization workflows](docs/guides/OPTIMIZATION.md) add host-declared intent guards,
typed prepared query tools, dependency-complete evidence selection, host-assisted rendered
token counting, local-match routing, readable-choice responses and offline configuration
comparison. Existing formats and defaults remain unchanged. Six local Qwen token probes
for 2/4/8 choices measured 158→118, 224→174 and 344→274 prompt tokens with the readable
codec, including choice/schema grounding. This is opt-in; smaller prompts alone do not
prove better decisions. The [tested example](examples/packet/optimized-turn.rs) shows intent
guarding and query arguments without SQL substitution. New API spend $0.

## Integrating a game (v0.2.0a)

The optional [integration kit](docs/guides/KIT.md) registers data meanings, units, visibility,
tools and named context recipes once. Build each turn from a validated snapshot, inspect
selection/budget decisions separately, and dispatch model tool calls through validated
host callbacks. [The runnable Rust example](examples/kit/registered-project.rs) includes Rust
builders, schema-only queries and shared brain feature units.

`gamectx init <new-directory>` generates a project, snapshot, recorded-run evaluation
suite and runnable starter checks. `doctor` checks registration; `inspect` explains a
recipe's packet. Persistent JSON-lines and a separate native C ABI have Python,
Node/TypeScript and C# adapters in [bindings](bindings/README.md).

Offline: 144 Rust/Python workflows agree, with 47 new conformance cases and 20,000 text
mutations. Reusing registration instead of rebuilding each turn measured 146,130,805 →
78,931,400 ns per 1,000 synthetic turns (seven paired release trials). These changes make
integration and validation easier; new model accuracy has not been measured. API spend $0.

## Local 1B–3B workflows (v0.2.0a)

[Local-model APIs](docs/guides/LOCAL.md) add full-request budgets with a host tokenizer callback,
protected scene facts and whole dialogue exchanges, legal-pair decisions with one retry,
and explicit deployment-bound guidance/format profiles. Existing filtering, arithmetic,
queries, facts and action validation do the local work. The core remains dependency-free.
See the [workload guide for 1B, 1.5B, 3B, 8B, 30B and 70B](docs/guides/MODEL_SIZES.md),
[tested example](examples/packet/local-turn.rs) and [local evaluation report](docs/evidence/LOCAL_MODEL_2026_11_06.md).

## Latest alpha.3 model check

On six fresh synthetic states across cargo, teams and routes, 48 questions per model
were tested on Llama 3.2 3B and Mistral Small 3.2 24B. Every format received the same
field meanings. Query mode includes native tool execution, at most one correction and
the model's final answer; all calls count toward the input averages below.

| Method | Correct / 96 | Average completed-exchange input tokens |
|---|---:|---:|
| Full JSON | 27/96 | 1,786.792 |
| Lossless packed tables | 27/96 | 1,252.125 |
| Schema + query tool | 91/96 | 916.490 |

Packing used 29.9% fewer input tokens at the same aggregate score. Schema + query used
48.7% fewer, scoring 46/48 on Llama and 45/48 on Mistral. First queries scored 83/96;
corrections raised this to 91/96, with zero correct-result copying errors. The five
remaining misses were extra limits/filters and an unsupported subquery. JSON and packed
scores tie overall but differ on individual cases.

Total usage-priced spend was **$0.05529491875**, including the initial token check and
one interrupted runner call. These are small synthetic tabular tasks, not live gameplay
or a test of brain/memory quality. [Full method, per-family scores, limitations and raw
responses](docs/evidence/MODEL_TEST_2026_11_06.md).

## Earlier model measurements

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
came from filtering questions. The [eval script](eval/query/model_eval.py) reruns this for about
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
- **Keep query data local.** On three held-out states and the two small models, schema-only
  requests with one correction scored 172/174 versus 171/174 with full tables. Including
  query, correction and final-answer turns, average input fell from 4,898.632 to 1,615.874
  tokens (67.0% less). Completed-exchange cost fell 64.1%. These are synthetic states;
  the one-answer accuracy difference is not evidence of general superiority.
- **Learn small local reaction policies.** The `brain` module improved agreement with
  reference reactions from 545/1,536 at random initialization to 1,421/1,536 after training
  across four synthetic tasks and unseen samples. Adapting a trained trading preference
  improved agreement from 264/384 to 384/384 on fresh samples. These are offline policy
  tests, separate from the LLM accuracy measurements above.
- **Select training rates using validation reactions.** On a separate paired test with
  fresh samples, opt-in rate selection scored 1,487/1,536 versus 1,436/1,536 for one rate.
  The nonlinear task improved from 315/384 to 359/384. This used about three times the
  estimated training work; inference is unchanged.
- **Do numeric filters in code, or ask for brief reasoning.** Questions like "items with
  count at least 15" were answered correctly 1 time in 27 across all formats. Asking the
  model to list candidate rows before its final answer raised filtering accuracy from
  37/54 to 46/54 for about 60 extra output tokens.

[docs/GUIDE.md](docs/GUIDE.md) turns these results into practical rules.

## What it does

From the included [Ember Keep example](examples/fixtures/ember-keep) (8 actions, a 10-item inventory,
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
- **Reaction brains** (`brain`): small, game-defined neural graphs evaluated locally, with
  guarded supervised training, bounded weight lessons, pins, revision/replay checks,
  saved learning and undo. The host supplies observations, labels and legal reactions.
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
- **Token estimates**: uncalibrated packet estimates count one byte per token; provider
  reports teach a per-model ratio with a 2x margin. These remain estimates, not a guarantee
  for hidden chat/tool framing. The optional [full-request contract](docs/guides/LOCAL.md) counts
  rendered requests and reserves output/safety separately.
- **Verification**: `gamectx verify` unpacks every lossless section and checks it matches
  the original exactly, or for a table with a view, the rows the view selected. Numbers
  keep their original text (`1.50` stays `1.50`) unless a `round` view changes them.

## Quick start

```bash
# v0.3.0a is the GitHub tag; Cargo package version is 0.3.0-a.
cargo install --git https://github.com/pink-kitsune-420/gamectx --tag v0.3.0a --locked
# From crates.io:
cargo install gamectx --version '=0.3.0-a' --locked
# or, from a clone:
cargo build --release
./target/release/gamectx report examples/fixtures/ember-keep/manifest.json
./target/release/gamectx pack examples/fixtures/ember-keep/manifest.json --receipt
./target/release/gamectx verify examples/fixtures/ember-keep/manifest.json
./target/release/gamectx search examples/fixtures/ember-keep/catalog.json "lanterns" --count 3
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

The formats are defined by 747 shared [conformance cases](conformance/README.md), each an input
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

Pin the release tag in `Cargo.toml` (package version `0.3.0-a`):

```toml
[dependencies]
gamectx = { git = "https://github.com/pink-kitsune-420/gamectx", tag = "v0.3.0a" }
```

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
turn.memory("memories", &memory, player_said, 8)?;   // gamectx::knowledge::memory::Memory

let packet = turn.build()?;        // packet.text, and packet.receipt of what was dropped
let system = packet.system_prompt("You are the game logic for Warden Ilse.");
```

`tools` and `inventory` are `gamectx::Value`s, from `gamectx::data::json::parse` on your JSON or
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
you need full control; run `cargo doc --open` from the v0.3.0a checkout for its API docs.

### Give the model a query tool

Include `gamectx::data::query::tool()` with your actions, then execute its `sql` argument over
the tables the game allows it to read. Return the JSON result, or the error so the model
can correct its request. Table names must match those shown in the packet.

```rust
use gamectx::{data::json, data::query};

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

### Keep rows in the game and allow one correction

`query::describe(&tables)` returns column types and complete small string domains, with
no rows or numeric samples. Send it as a per-turn `query_tables` variable alongside
`query::tool()`. Define field meanings explicitly: `unit_weight` is per item;
`stack_weight` is `count * unit_weight`, computed by the host using `derive`.

```rust
use gamectx::{obj, data::query};

let mut session = query::Session::new().result_bytes(512);
let feedback = session.apply_call(&obj! {"sql" => "SELECT SUM(stack_weight) FROM food"}, &tables)?;
assert_eq!(feedback.get("retry").and_then(gamectx::Value::as_bool), Some(true));
// Send feedback to the model. It gets table descriptions and one correction allowance.
let result = session.apply_call(
    &obj! {"sql" => "SELECT SUM(stack_weight) FROM inventory WHERE kind = food"}, &tables)?;
assert_eq!(result.to_json(), r#"{"result":23}"#);
assert!(session.is_finished());
```

Here `tables` contains the host-derived `stack_weight` column. Use one session per
question over the same snapshot. Success ends it too: a valid query with the wrong
meaning does not trigger a retry. See the [runnable host example](examples/data/query-workflow.rs):

```bash
cargo run --example query-workflow
gamectx query --schema tables.json
# Each stdin line is a query tool argument object, e.g. {"sql":"SELECT COUNT(*) FROM inventory"}.
gamectx query --session tables.json --result-bytes 512
```

### Inspect and bound a query locally

The following additions are included in v0.2.0a.

`Query::parse` can be reused on later snapshots. Query execution now borrows input rows,
streams aggregates and copies only selected output cells. Five synthetic paired release
fixtures measured 4.6–32.5× faster execution, with the same results and error messages;
this measures local execution, not model latency. [Measurements](eval/README.md#query-execution-and-identifier-matching).

`Query::plan()` explains table, aggregate use, columns, filters, order and limit.
`Query::run_bounded(rows, max_rows, result_bytes)` refuses oversized inputs/results.
`query::Shape` optionally enforces a host-declared table, filter-column multiset, limit
and aggregate flag. It does not verify natural-language intent, filter values, selected
fields or permissions. Use it for structured host requests with known requirements.
The [runnable example](examples/data/query-plans.rs) is tested:

```bash
cargo run --example query-plans
gamectx query --explain "SELECT SUM(mass) FROM cargo ORDER BY mass DESC LIMIT 3"
```

Optional `query::Guidance::ExplicitSelection` supplies a supplemental system paragraph;
`Standard` (empty supplement) remains the default. On six fresh synthetic states the
3B model improved 46/48→48/48, while 24B fell 47/48→46/48; complete-exchange input rose
22.0%. Choose per model/workload rather than assuming all weaker models improve.
See the [paired report](docs/evidence/QUERY_OPTIMIZATION_2026_11_06.md) and tested example above.
CLI: `gamectx query --guidance explicit-selection` prints the paragraph as a JSON string.

Table and catalog `relevant_to` recognize namespace and dotted ID components, including
`grove:salt_scroll`, `quest.gate.open` and Unicode identifiers. Ten lexical regressions
improved from 2/10 to 10/10; original values and number spellings are preserved.
This is a lexical search improvement, not a model-accuracy measurement.

## A local brain for your game's reactions

Names and meanings come from your game. Bind normalized numeric observations (0–1),
provide example utility scores, and choose which reactions are currently legal. The
brain ranks them locally; game code validates and executes the chosen action. Missing
inputs remain unknown, and ties or unknown eligible scores yield `choice:null` for a
host-defined fallback. Scores are utilities, not confidence probabilities.

```rust
use gamectx::{runtime::brain::Brain, obj};

let sample = |danger: f64, need: f64| obj! {
    "inputs" => obj! {"danger" => danger, "need" => need},
    "targets" => obj! {"withdraw" => danger, "assist" => need * (1.0 - danger)}
};
let mut brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
let report = brain.train(&obj! {
    "id" => "first_training", "expected_revision" => 0,
    "examples" => vec![sample(0.1,0.2),sample(0.2,0.8),sample(0.8,0.1),sample(0.9,0.7)],
    "validation" => vec![sample(0.15,0.75),sample(0.75,0.15)], "epochs" => 128, "rate" => 0.1
})?;
assert_eq!(report.get("accepted"), Some(&gamectx::Value::Bool(true)));
let prediction = brain.run(&obj! {"danger" => 0.8, "need" => 0.2}, &["withdraw", "assist"])?;
assert_eq!(prediction.get("choice").and_then(gamectx::Value::as_str), Some("withdraw"));
let restored = Brain::restore_json(&brain.to_value().to_json())?;
assert_eq!(restored.to_value(), brain.to_value());
```

The labels above use a tiny reference policy for demonstration. Real hosts supply
designer labels or observations mapped to desired utility scores. Candidate training
is retained only when mean squared error falls on a separate validation set. Rejected
updates leave the entire brain unchanged. A validation improvement checks the supplied
labels; evaluate actual gameplay quality separately.

An AI can propose graphs with `brain::tools()` and `brain::create_call`, then use
`Brain::apply_call` for `inspect_brain`, `run_brain`, `teach_brain`, `train_brain` and
`undo_brain`. Tool requests are bounded JSON text that packs as signatures. The host
chooses each brain's owner, allowed feature bindings and whether to install a proposal;
model-proposed examples are synthetic labels, not observed facts. Normal runtime inference
uses native observations and makes no model calls. Keep training on the host's schedule.

```bash
cargo run --example adaptive-brain
gamectx brain examples/fixtures/brain/workflow.json
gamectx brain --tools
```

Graphs have at most 16 inputs, 32 nodes and 16 outputs. Training, edits, retained history
and parameter displacement are bounded. Integer millionths make inference, training and
saved output portable; see [the brain format](docs/FORMAT.md#reaction-brains) and
[offline measurements](eval/README.md#local-reaction-brains). Use exact formulas/queries
for exact rules and these small networks for learned preferences. This is a reusable
reaction-policy component; text understanding, temporal memory, engine adapters and
reinforcement-learning reward assignment remain host concerns.

### Select for reaction accuracy

`Brain::train_select` compares a bounded list of learning rates on candidate copies.
It prefers more correct validation reactions, then lower validation MSE. It requires
both lower MSE and no reaction-accuracy regression relative to the active brain. One
winner is retained with one revision and undo snapshot; rejected trials leave no history.

With host-supplied `examples` and separate `validation` in the same format as above:

```rust
let report = brain.train_select(&obj! {
    "id" => "selected_training", "expected_revision" => brain.revision(),
    "examples" => examples, "validation" => validation, "epochs" => 128,
    "rates" => vec![0.03,0.1,0.3], "max_steps" => 2_000_000
})?;
```

The explicit aggregate work cap is at most 2,000,000 units; each of at most eight trials
also obeys the original 500,000-unit cap. The original `train` stays available for
single-rate MSE training. The model tool is `train_select_brain`; a runnable workflow is:

```bash
gamectx brain examples/fixtures/brain/selection.json
```

This metric assumes all outputs are eligible and zero score/margin thresholds. Check
host gameplay metrics separately when using state-dependent eligibility or other thresholds.

### Preload brains and cache repeated states

Load and validate saved learning once at startup. Retain the compiled `Brain`, or clone
a template for actors that should learn independently. `CachedBrain` adds an optional
bounded prediction cache:

```rust
use gamectx::runtime::brain::{Brain, CachedBrain};
use gamectx::obj;

let saved = include_str!("examples/fixtures/brain/preloaded.json");
let template = Brain::restore_json(saved)?;
let mut actor = CachedBrain::new(template.clone(), 32)?;
let other = CachedBrain::new(template, 32)?;
let input = obj! {"danger" => 0.8,"need" => 0.5};
let result = actor.run(&input, &["withdraw", "assist"])?;
assert_eq!(actor.run(&input, &["withdraw", "assist"])?, result);
assert_eq!(actor.stats().hits, 1);
actor.brain_mut().undo(1)?;
assert_eq!(actor.stats().entries, 0);
assert_ne!(actor.run(&input, &["withdraw", "assist"])?, result);
assert_eq!(other.brain().revision(), 1);
```

Capacity is 0–256 entries; zero disables caching. Keys include normalized inputs,
eligible reactions, thresholds and revision. Mutable access clears entries before
training, undo or replacement, including replacement at the same revision. Saved
learning excludes cached predictions. The host still supplies fresh observations and
checks actions against the current world.

In the [offline release benchmark](eval/README.md#brain-preloading-and-prediction-caching),
20,000 predictions over repeated states took 15.939–19.223 ms cached versus
29.161–36.524 ms uncached across two network sizes and two reuse patterns
(1.7–2.0× faster). With no hits, caching was 1.8–1.85× slower. Check `stats()` for your
workload; caching is opt-in. Cloning a compiled template was 15.9–17.2× faster than
restoring the same saved JSON per actor in this local measurement.

```bash
cargo run --example preloaded-brains
gamectx brain examples/fixtures/brain/workflow.json --cache 32
```

The CLI cache lives for one workflow; output and saved state remain byte-identical.
API spend for these measurements: $0.

### Keep work within a frame

`Brain::run_into` accepts fixed-point input arrays and an output eligibility mask, and
writes a reusable `Prediction`. It performs no heap allocation on successful calls:

```rust
use gamectx::runtime::brain::{Brain, Prediction};
let brain = Brain::dense(&["danger", "need"], &["withdraw", "assist"], 4, 3)?;
let mut prediction = Prediction::default();
brain.run_into(&[Some(800_000), Some(200_000)], 0b11, 0, 0, &mut prediction)?;
assert_eq!(prediction.scores().len(), 2);
```

Inputs, scores and thresholds use integer millionths (`SCALE = 1_000_000`); `None` is
unknown. Choice indices follow `Brain::outputs()`. The JSON APIs remain available and
produce identical results. A local release benchmark of 20,000 changing-state calls
measured 26,030,769 / 32,503,158 ns through JSON versus 1,968,370 / 6,171,925 ns natively
(small/large networks), with zero native allocations. These compare API paths, not
additional neural accuracy or universal game-loop speedups.

`brain.start_train`, `start_train_select` and `start_train_select_host` create disposable
`TrainingJob`s. Call `advance(work_budget)` each frame, then `finish(&mut brain)` when
complete. A slice stops at a sample boundary; `next_steps()` reports its minimum budget.
Dropping a job cancels it. Finish checks the entire starting state before committing;
changed or replaced brains reject stale jobs. Training order, reports and saved results
match uninterrupted execution. Work units are deterministic estimates, not milliseconds.

`schedule::Scheduler` staggers actor work, responds to change/urgent events, refreshes
idle actors and enforces an estimated per-frame budget. Hosts notify it when observations,
brain revisions, eligibility or thresholds change and execute the returned reservations.
In the synthetic 64-actor/240-frame check, it scheduled 512 evaluations versus 15,360
per-frame evaluations, peaking at eight per frame. Overload can delay refresh deadlines.

`train_select_host` uses validation rows with `inputs`, `targets`, `allowed`, `min_score`
and `min_margin`; it compares actual choice/abstention to the target utilities under
those same settings. Target ties, zero utilities and threshold failures expect abstention.
It retains one candidate only when MSE improves and this decision count does not regress.
On fresh masked/thresholded test seeds, both selection methods scored 1,466/1,536;
this adds an appropriate host metric, with no measured accuracy gain on that cohort.

### Reuse packet bytes and synchronize context

`Context` now retains serialized registered sections. Byte-budget assembly sizes sections
without allocating temporary JSON and serializes the final packet once. For repeated
assembly outside `Context`, use `budget::PreparedSection` and `assemble_prepared_bytes`.
In a local 500-call benchmark with budget pressure, the previous algorithm took
146,774,066 ns, one-shot assembly 19,042,995 ns and reused prepared sections 1,834,731 ns.
One-shot packets that already fit were about 9% slower in this run, with fewer allocations;
prepared reuse is the preferred path for repeated builds. Arbitrary fitting
predicates still inspect each complete candidate; token estimates are never treated as
additive byte counts.

`sync::Publisher` / `Replica` transfer a full snapshot initially, then changed sections
when the receiver explicitly acknowledges the current revision. Lost or mismatched
acknowledgements produce a new snapshot. Receivers reject replay, wrong streams/baselines,
invalid removals and oversize input atomically, while preserving exact JSON numbers/order.
A 120-turn synthetic trace transferred 19,089 bytes versus 489,490 full-state bytes.
These are transfer bytes: a model receives reconstructed state unless its host implements
the protocol. Deltas do not guarantee lower inference tokens or context-window usage.

```bash
cargo run --example runtime-loop
python3 -I eval/query/prefix_cache_eval.py --cap 0
# Optional: a running local llama.cpp server; no credentials or paid provider.
python3 -I eval/query/prefix_cache_eval.py --url http://127.0.0.1:8080 --cap 0
```

The [complete host example](examples/runtime/runtime-loop.rs) combines scheduling, sliced
training, native inference, prepared context and acknowledged synchronization. The prefix
harness compares local server cache modes and records reported token counts/timings;
actual server KV-cache savings remain unmeasured here. All new measurements cost $0.
See [reproduction details](eval/README.md#frame-work-and-context-reuse).

### Bind inputs, explain decisions and replay difficult states

`brain::FeatureSchema` declares input names/order, units, physical ranges, inversion and
missing/out-of-range policies. `BoundBrain` saves that schema with learned state and
requires the complete expected schema when restoring. Changing units, ranges, versions
or input order is rejected. Normalize physical values into fixed-point `Features` before
native inference; unavailable values remain unknown unless the schema explicitly errors.

`Brain::diagnose` adds deterministic reasons such as `unknown_eligible`, `tie`,
`score_threshold` and `margin_threshold`. `replay::Recorder` captures normalized inputs,
eligibility, labels, optional measured latency and explicit brain-state changes. Replay
reports decision agreement, abstention/switch counts and recorded latency percentiles.
It checks at most 256 events/1 MiB; it does not simulate world consequences or explain
an actor's intentions. Keep a versioned binding envelope with traces when physical input
meaning matters. The [combined example](examples/runtime/integration-tools.rs) runs as a test.

`StableBrain` optionally keeps the previous eligible reaction until a new winner improves
utility enough or its minimum hold period ends. Urgent events bypass holding; ineligible
choices and normal inference abstention always take effect immediately. The reusable
`Selection` exposes both raw prediction and selected reaction, so their meanings stay
clear. Mutable brain access resets holding state. Three fresh 240-frame noisy trajectories
produced 355 raw switches versus 15 stable switches, with zero missed urgent switches and
zero ineligible choices. Holding sacrificed 2,281,674 total utility millionths across 720
frames (about 0.003169 utility/frame). Evaluate response quality alongside consistency.

### Supply meanings when field names are ambiguous

`query::describe_with(tables, metadata)` adds bounded host-authored field descriptions,
units, kinds and literal meanings; `describe` and query execution stay unchanged.
For example, rarity `unique` is a category independent of quantity, while mass per item
and mass per stack require different totals. Metadata is checked against table names,
columns, types and supplied literal values.

On three fresh 24-row states, 30 questions/model, meanings improved executed-query
accuracy from 36/60 to 45/60: Llama 3.2 3B 12/30 → 18/30, Mistral Small 24/30 → 27/30.
Token-only input counts rose 205 → 365 and 190 → 352 respectively. This is a targeted
semantic test, not a general gameplay benchmark; metadata costs tokens and still leaves
incorrect queries. Both runs together cost $0.0053331125. [Method and recorded queries](eval/README.md#bindings-semantics-and-evidence).

```bash
gamectx query --schema examples/data/query-meaning-tables.json --metadata examples/data/query-meanings.json
gamectx replay examples/fixtures/brain/trace.json
cargo run --example integration-tools
```

### Remember observations and reason with evidence

The existing `Memory` keeps its original event-log behavior. `memory::Journal` adds scoped
entity/key facts, observed/proposed provenance, expiry, permanent supersession and explicit
forgetting. Active observations take precedence over proposals; recalling after a newer
fact expires never revives the older superseded observation.

`memory::EvidenceMemory` adds separately labeled hypotheses with supporting and contradicting
observation IDs. The host records actual evidence and retains a sealed `EvidenceSnapshot`
while a model sees its JSON payload. Applying a proposal rechecks the originating instance,
revision, actually supplied citations, current source availability, clock and privacy.
Hypotheses never become native observations or cite each other as evidence.

A proposal contains `id`, `statement`, `confidence`, `supports`, `contradicts` and `ttl`.
Confidence is an uncalibrated declared score, capped at 0.95. Hypotheses remain `tentative`
or `contested`; repeats with the same evidence cannot raise confidence, extend expiry
or reword the claim. Lifetime is also capped by expiring sources. The host can contradict
evidence, filter snapshots by privacy, and physically forget evidence plus dependent
hypotheses. Snapshots fit an explicit byte budget, keeping references synchronized with
retained evidence. Restores require matching subject and clock identity.

```bash
cargo run --example evidence-memory
```

The [tested example](examples/knowledge/evidence-memory.rs) records two observations, stores a
contested hypothesis, checks persistence and removes a private dependency. The offline
reference accepted six valid workflows and rejected nine invalid workflows. Repeated
proposals kept confidence at 0.6, expiry at turn 30 and native observations at two.
These are integrity checks; citation validation does not establish that evidence logically
supports a claim. The host supplies authoritative observations, time and consent.

The fitting-packet regression was also addressed: a paired release benchmark measured
1,000 fitting builds at 9,171,479 → 5,905,360 ns and mixed builds at
8,822,089 → 5,708,635 ns. Pressure-path allocations increased 153 → 172/build while
runtime improved; full timing/allocation/peak-memory results are in [the eval guide](eval/README.md#bindings-semantics-and-evidence).

### Register facts explicitly

`facts::FactRegistry` stores typed assertions under a scope and key: world rules, user
preferences, engine state or host-validated conclusions. User assertions take precedence
over host updates; pinned facts require explicit unpinning. Replacements require the
current revision, preventing an old reply from silently overwriting newer information.

Derived facts can cite exact source revisions. Updating, forgetting, expiring or hiding
a source makes its descendants unavailable. Privacy labels propagate to descendants;
privacy erasure physically removes their payloads too. Registration remains an explicit
host/user operation; model ideas belong in evidence memory until the host validates them.

`check` compares a typed claim with the current visible assertion and returns `supported`,
`conflicting` or `unknown`. Decimal comparison preserves large integer IDs and recognizes
equivalent numeric spellings without float rounding. This checks agreement with registered
assertions; it does not independently establish their truth or understand arbitrary prose.

Use `Turn::facts` to add a budgeted table to context, and direct `get`/`check` calls for
facts omitted from the packet. The offline fixture registered and checked 128/128 facts;
a 1,024-byte packet cap retained eight in 927 bytes, reporting 120 dropped. These are
mechanical checks, not a measured increase in model accuracy. API spend: $0.

```bash
cargo run --example registered-facts
```

The [tested example](examples/knowledge/registered-facts.rs) covers registration, pins, claim checks,
dependency invalidation, persistence and packet assembly. [Format and limits](docs/FORMAT.md#registered-facts)
and [offline reproduction](eval/README.md#registered-facts) describe the contract.

### Reuse knowledge and computations in the game loop

Seven optional APIs extend the existing building blocks; default recall, tools and brain
formats keep their behavior:

| API | Purpose |
|---|---|
| `recall::RecallIndex` | Build a lexical index once; optional word normalization and host aliases, with no unrelated fallback. Rebuild after source/privacy changes. |
| `playbook::Playbook` | Return reusable action drafts only when exact fact versions/values, expiry, privacy and a current host action validator pass. |
| `outcome::OutcomeTracker` | Require execution and observation opportunity before accepting a host score; consume a training label once. Unseen trials remain unknown. |
| `temporal::TemporalWindow` | Count distinct native events, rates, numeric changes and ordered sequences within bounded windows. Missing observation coverage stays unknown. |
| `brain::BrainBundle` | Prepare perception once per scope/frame and reuse its normalized output ports across candidate decisions. Modules train independently; replacements invalidate prepared frames. |
| `requests::RequestGate` | Consume sealed result tickets once, checking actor/world, schema/permission revisions, fact versions, privacy and expiry. |
| `spending::SpendingLedger` | Reserve integer microdollars before dispatch; settle usage, cover concurrent requests/retries, and retain worst-case charges after interrupted requests. |

Offline selective recall: 100 queries over 1,000 records visited 100 index postings versus
100,000 record visits for a full scan (index construction excluded). Two normalization/alias
examples matched 2/2; broader retrieval precision remains unmeasured. A repeated validated
procedure matched 100/100 checks; this measures local reuse opportunities, not avoided
provider calls in a live game. Outcome fixtures returned 100 observed labels, zero trainable
unexposed trials and zero duplicate labels.

With 12 candidates on each of 100 frames, shared perception reduces module evaluations
2,400→1,300. All 3,600 candidate predictions on fresh seeds 9101/9102/9103 match duplicate
evaluation exactly, including unknown inputs. A seven-trial paired release benchmark for
12,000 candidates measured 729,906→631,514 ns with no hidden nodes and
3,341,042→2,014,761 ns with 16 hidden nodes/module. Workload and hardware matter;
this is not a measured game-frame or model-accuracy improvement. All new checks cost $0 API.

```bash
cargo run --example runtime-upgrades
```

The [tested synthetic example](examples/runtime/runtime-upgrades.rs) combines all seven APIs.
See [the guide](docs/GUIDE.md#reuse-and-checked-runtime-work),
[format contracts](docs/FORMAT.md#optional-runtime-upgrades) and
[measurements](eval/README.md#optional-runtime-upgrades). The host retains world sensing,
permissions, action execution, durable storage, provider pricing and request dispatch.

## Be honest with your numbers

- **Bytes saved are not tokens saved.** In the run above, gamectx cut bytes by 58% but
  tokens by 46%. Measure with your provider's reported usage, which is what `calibrate`
  is for.
- **Results depend on your data and models.** The original packing benchmarks use one
  state and three models; query workflows cover four states and the two smaller models.
  Test the decisions your game actually depends on, with the eval script or your own.
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

## Living world and one-file cast packs

The opt-in [living-world layer](docs/characters/LIVING_WORLD.md) loads a shared setting, locations,
groups and multiple archetypes from one validated file. It propagates attributed reports
and bounded group influence without model calls, selects relevant habit/atmosphere context,
and provides immediate authored speech with an optional validated upgrade before delivery.
Run `cargo run --example living-world` and edit [the town pack](examples/fixtures/cast/chime-town.json).
Evidence covers 54 independent Python/Rust workflows and a retained 1,000-frame host-work
trace. Existing outputs stay unchanged; this adds no model dependency or whole-game FPS claim.

Long-session diagnostics and opt-in transient dialogue retention are documented in
[the overnight evaluation](docs/evidence/OVERNIGHT_DEEP_EVAL.md). Routine Web chat no longer
consumes rumor roots; the retained 600-turn offline trace completes with zero rejections.
Archive-aware `Life::recall_selected` adds explicit subject filters and required causal
anchors while preserving legacy recall. Its bounded, host-selected policy is explained
in [the guide](docs/GUIDE.md).
The Studio's bundled configuration uses authorized factual speech variants; its Speech
policy selector also offers free dialogue. Provider output is validated against current
host cards, and focused requests scope recalled episodes/promises to the active player.
The Studio also supports bounded polite commands and scoped pronouns, with action-button
clarification for ambiguity. Scene-generation receipts reject delayed controls after a
participant switch; delivery/recognition continue using their own speech/session IDs.

For question-specific factual answers, the optional required-answer policy removes generic
deferral while retaining source checks. A fresh restored-cast paired 3B check improves
required-answer coverage from 3/12 to 12/12 using host-authored sentences. The completed
[overnight report](docs/evidence/OVERNIGHT_DEEP_EVAL.md) includes raw failures, bounded-storage
limits, tier audits and the distinction from free-model intelligence.

The local Studio now shows applied model status, installed Ollama names, request activity
and offline/model reply labels. Use **Model setup** to check and apply a model, and
**New game** to reset play while retaining settings, saves, approved art and spending.
No model setup is required before meeting a character.

Lantern Vault now includes an opt-in **Models & Services** manager for demand loading,
idle release, predictive warming and owned service controls. Configurable **Dramatic scene**
rules select separate text/image profiles and a detailed art bible; online profiles use
the existing spending ledger. [Configuration and evidence](docs/studio/MODEL_MANAGER.md).

Native Piper/whisper.cpp support uses separately installed engines and weights. See [setup](docs/studio/NATIVE_VOICE.md); model and voice controls live in Settings.

Two-player LAN chat keeps AI and world processing on the host; see [LAN play](docs/studio/LAN_PLAY.md).

Piper prepares full lines before playback; see [buffered speech and microphone checks](docs/studio/BUFFERED_SPEECH.md).

Saved voice recordings and floating context/execution profilers: [guide](docs/studio/STUDIO_DIAGNOSTICS.md).

The local studio can opt into [saved character viewpoints](docs/characters/CHARACTER_VIEWPOINTS.md) for dreams, hobbies and preferences; confirmed subjective speech stays separate from world authority.

[Host-approved character reactions](docs/characters/CHARACTER_REACTIONS.md) add optional personality/attitude/resource gates, generic handler authorization and playable coin/item loans, gifts and support at zero extra model calls.

[Studio social turns](docs/studio/STUDIO_CONVERSATION.md) route greetings, wellbeing and heard-topic follow-ups separately from factual shop answers when personality turns are enabled.

[Contextual greetings](docs/characters/CONTEXTUAL_GREETINGS.md) remember confirmed introductions per NPC/player and adapt to encounters and places. Entering a room or using Approach can open conversation through a stage direction, with zero model calls.

## Optional reasoning and automatic speech

See [deliberation controls](docs/studio/STUDIO_DELIBERATION.md) for opt-in, capability-aware native reasoning, unverified notes and per-browser automatic playback. Existing formats and default outputs are retained.

## Longer conversations and registered roleplay actions

See [conversation extensions](docs/studio/STUDIO_CONVERSATION_EXTENSIONS.md) for bounded sentence tickets, longer player input, unverified optional summaries and `RoleplayActions` registration. Host validation and spending caps still apply.

With [AI character wording](docs/studio/STUDIO_EXPRESSION.md), connected models word greetings,
recall and factual replies as well as personal conversation; matching validated wording
can be reused, and authored lines cover offline/failed generation. Original tiny tales,
scoped name/preference disclosures and oversized-request negotiation have separate
contracts. Try `cargo run --bin gamectx -- local examples/fixtures/cast/expression-fixture.json`;
that editable packet is checked in the API tests. See the [roleplay investigation](docs/research/ROLEPLAY_RESEARCH.md)
for public research, original implementation choices and measured limitations.

Current-place dialogue, twelve plot seeds per NPC with heard-history rotation, and retryable
unheard audio are documented in [scene/delivery repairs](docs/studio/SCENE_SOCIAL_REPAIR.md).
The [social-character audit](docs/research/SOCIAL_CHARACTER_GAPS.md) maps identity, senses,
implicit memory, relationships, motives, actions and society to current capabilities and gaps.
