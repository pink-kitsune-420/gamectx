# Lantern Vault — playable gamectx testbed

An original six-room text dungeon, implemented with Rust's standard library and gamectx.
It needs no model, network, assets or dependencies to play. This is an **unreleased example**
for the current checkout, not part of the existing v0.2.0a tag.

```sh
cargo run --example dungeon -- --seed 7
```

Recover the glass star and escape. Explore, talk to the keeper/merchant, obtain a bronze
key and oil, unlock the vault, fight the sentinel, take the star and leave. Every screen
lists the currently legal commands. `look`, `help`, `state`, `packet`, `packet named` and
`quit` are available. Targets accept either the full ID or its short suffix:

```text
go gallery
go workshop
buy oil
take bronze_key
go gallery
inspect vault
unlock vault
go vault
attack sentinel
```

Keep attacking until the sentinel falls, then `take glass_star` and `go exit`. That route
is for seed 7; other seeds can put the key in the cistern. Inspecting a lock never opens
it. The keeper's seal belongs to the keeper and cannot be taken. Expensive tonic cannot
be bought without enough coins. Combat damage, prices, affordability, ownership and all
world changes are resolved locally. No model narration can change game state.

Seeds 0..65535 vary the key location, purse, prices and sentinel health. The map is fixed.
A run permits at most 256 accepted actions. The game is deliberately small; it is an
integration/regression environment, not a claim to represent every game's difficulty.

## Deterministic replay

```sh
cargo build --example dungeon --locked
./target/debug/examples/dungeon --replay examples/dungeon/win.json
python3 -I eval/dungeon_cases.py --cap 0 --check --audit eval/dungeon-results.json
```

A replay is `{seed,commands:[{revision,decision:{action,target}}]}`. A null revision uses
the current revision; explicit old revisions test stale results. Up to 256 commands and
262,144 input bytes are permitted. Output contains every result and public state, final
state and win status. Rejections leave state/revision/history unchanged. Malformed replay
structure fails the request; well-framed illegal actions are recorded as rejections.
The bundled `win.json` wins seed 7 in 13 accepted actions.

The independent Python rules agree byte for byte with Rust on 103 workflows: 101 accepted
and two rejected replay files, including 80 generated 30-command walks. Twenty-three separate example
conformance fixtures in `examples/dungeon/cases` cover wins, theft, unaffordable purchases, inspection, stale proposals,
early relic grabs, malformed actions, terminal runs and bounds. They add no core operations or format
changes. Mutation tests add 10,000 attempts. Six scripted seeds all escape; those are
scripted rule checks, not model accuracy.

## JSON-lines host

```sh
./target/debug/examples/dungeon --serve --seed 7
```

One bounded UTF-8 request per line, at most 16,384 bytes. Oversized/invalid-UTF-8 lines
terminate the process, so no unbounded recovery allocation is needed. A normal malformed
JSON request produces an error reply and the next line may continue. EOF closes cleanly.
The game lives in one persistent process; a new process starts a new seeded run.

```json
{"version":1,"id":"first","op":"observe","args":{"input":"Recover the glass star and escape.","mode":"named","context":2048}}
```

Replies are `{version:1,id,ok,result}` or `{version:1,id,ok:false,error}`. Invalid framing
has a null ID. Operations:

| op | args | Result |
|---|---|---|
| state | `{}` | Current public state and legal pairs |
| observe | `{input,mode,context}` | Model request, budget/section/dialogue receipts and host diagnostics |
| act | `{revision,decision:{action,target}}` | Validated local action result |
| decide | `{revision,text,format}` | Strictly decode `pair` or `named` model text, then validate/commit |
| query | `{sql}` | Read-only SQL over actor-visible `items`; host diagnostic interface |

Send only `observe.result.request` to a model. `inspection`, receipts and state are host
metadata. The mode is `raw`, `scene` or `named`; `decide.format` is `named` for named mode
and `pair` for the other modes. Do not automatically expose all host operations as tools.
The host executes sequentially and rechecks the current revision/legal pair at commit.
Repeated proposals with old revisions cannot double-purchase or consume items again.

## What this exercises

- One prepared kit registration, actor-visible snapshots and host-only field erasure.
- A typed affordability query on the recipe's view, rather than model arithmetic.
- Required current facts with complete evidence dependencies.
- Scene-scoped recent whole exchanges, optional older visible history, and drop receipts.
- Full-request budgets with output 96 and safety 256 tokens. Context 2K is an editable
  starting point; the example accepts 512..32768. Its JSON-byte/3+64 counter is explicitly
  **estimated**, not a tokenizer, calibration result or universal truncation guarantee.
- Schema-constrained action/target or readable named choices, followed by current host
  checks. The dispatcher validates argument schemas before callbacks.
- Required state preservation under small budgets and controlled failures when it cannot fit.

Both raw and scene requests contain only actor-visible information. Raw uses visible
object rows and optional older public snapshots; scene uses packed rows/current state;
named also uses the readable codec. All modes protect required facts and recent dialogue.
The raw archive may drop to fit; dropping is explicitly reported. No model gets an
oracle route, hidden key location, unvisited item list or host-only diagnostic field.

Across 80 matching scripted checkpoints for six seeds, request JSON totals were
275,304 bytes raw, 261,304 scene and 206,347 named. Raw dropped its optional archive at
55 checkpoints; scene/named had no optional archive. These byte totals are **not token
counts**, and the history/drop differences preclude attributing every byte difference
to table packing alone. Raw evidence is `eval/dungeon-results.json`, checked in CI.

## Optional local model player

Build the debug example first. Use an already installed local model; the runner never
pulls weights, reads credentials or contacts a hosted API. `--cap 0` is required.

```sh
python3 -I eval/dungeon_agent.py --cap 0 --tokens-only --model arzaan789/qwen2.5-3b-uncensored:latest --seeds 7 --mode named --output eval/results/dungeon-tokens.json
python3 -I eval/dungeon_agent.py --cap 0 --model arzaan789/qwen2.5-3b-uncensored:latest --seeds 7 --mode named --tokens eval/results/dungeon-tokens.json --max-turns 20 --max-calls 64 --output eval/results/dungeon-play.json
python3 -I eval/check_dungeon_agent.py --cap 0 eval/results/dungeon-tokens.json eval/results/dungeon-play.json
```

Other modes are `raw`/`scene`; default seeds are 0,7,101. Use new output files and rerun
token probes after changing mode, seed list, model/template or runner identity. The agent
requires a successfully completed matching initial token artifact. Each subsequent call,
including one optional correction, gets another native one-token preflight. Native prompt
counts are post-call observations, not proof an arbitrary oversized prompt avoided runner
truncation. Model calls are bounded globally by `--max-calls`; turns/seed by `--max-turns`.
Preflight and decision calls both count toward the cap. Failed proposals never execute.

Artifacts checkpoint full requests, raw response chunks, phases, usage, validation and
public before/after states. Complete-turn timing begins before observation/hydration and
ends after action/state validation, including native preflights and correction. Setup is
excluded. Runner memory is not sampled; use external game/runner/system memory monitoring
for performance studies. Natural caches, thermal state and concurrent workloads matter.
Different agents take different trajectories, so episode totals alone do not isolate a
format's effect. Pair packet/token tests at identical checkpoints and use new seeds and
request phrasing for generalization.

The first local smoke run is retained in `eval/dungeon-model-{tokens,results}.json`. It
uses the same pinned Qwen 3.09B Q4_K_M deployment as prior tests, named mode and seed 7.
It exposes a planning failure: legal repeated `look_entry` decisions do not advance the
quest. Success, validity, state changes and distinct rooms are audited separately; legal
JSON is not a win. This is one small diagnostic, not a model comparison. Background Rust
checks overlapped the run, so its timings are not a controlled latency benchmark. Token
and gameplay runs each spend $0 API. CI audits saved traces without generating anything.
