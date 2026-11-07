# Lantern Vault — playable gamectx testbed

An original six-room text dungeon, implemented with Rust's standard library and gamectx.
It needs no model, network, assets or dependencies to play. The example is included in v0.3.0a;
it is not present in the earlier v0.2.0a tag.

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

## Human player with local AI characters (v0.3.0a)

Keep the basic player above for deterministic rule testing. To play as the human while
NPCs generate their own replies, build the example and start the new AI mode:

```sh
cargo run --example dungeon -- --ai --size 3 --seed 7
```

This uses Python 3's standard library and an already installed Ollama GGUF model at
`127.0.0.1:11434`. The default tag is `arzaan789/qwen2.5-3b-uncensored:latest`; choose your
installed tag with `--model`. No model is pulled, no credentials are read, no hosted API
is called, and the runner configuration is not changed. The game reports an error if
the model is unavailable; use basic play to continue without AI.

Use `go gallery`, `go workshop`, then `talk merchant`. Introduce yourself and ask another
question; `/back` returns to exploring. `talk keeper` works at the cistern. `say <words>`
asks the nearby character once; `describe` asks the narrator about the current scene.
`/memory` inside a conversation shows original player quotes; `/forget` clears that
character's conversation. Everything is transient and ends when the process exits.

Natural-language actions go through a bounded interpreter. Exact listed commands need
no generation. Clear entity phrases narrow choices locally; overlapping phrases prefer
“keeper seal” over “keeper”. Ambiguous requests ask which target you meant. A recent
accepted action can resolve “it” only in the same room, at most four accepted turns old,
while the entity is still available. Moving rooms or completing a conversation clears
that action focus. Clarification selections require the same world revision. The human
confirms model-proposed world changes; purchase prompts show the current host price.
Arithmetic, availability, ownership and final action checks stay in Rust.

`--size 1`, `1.5`, `3`, `8`, `30` or `70` selects editable **task hints**, not a model or
measured capability limit. The context starts at 2048 for classes through 3B, 4096 for
8B, 8192 for 30B/70B; `--context` overrides it. Larger templates do not imply a model
fits 8 GB RAM. `--guidance focused` is the default separated workflow; `standard` and
`grounded` retain comparison prompts. See [WORKFLOWS](../../docs/WORKFLOWS.md) for exact
reservations, all 24 profiles, saved local measurements and known semantic failures.
If a required packet cannot fit, the turn fails instead of losing required facts.

For explicit generation/log limits, run the standalone front end directly:

```sh
cargo build --example dungeon --locked
python3 -I examples/dungeon/play_ai.py --cap 0 --size 3 --seed 7 --max-calls 64
```

Defaults: 128 calls, maximum 256; each native one-token preflight, generation or correction
counts, including failed dispatches. One correction is allowed per utterance. CPU-only,
four threads, temperature zero and identity checks match the local diagnostic setup.
`/stats` shows usage while exploring. `--log <fresh-file>` opts into full request/response
logs; otherwise conversations are not written to disk. Native prompt counts are observed
after dispatch; they do not prove an arbitrary oversized prompt escaped truncation.
The host first estimates the full request, then the runner probes and reserves actual
reported prompt usage plus output plus 256 safety before generation.

Each character scope stores up to 32 original player quotes and eight whole exchanges;
the narrator is scoped per room. Focused dialogue selects 1–3 recent exchanges and 1–6
recalled quotes according to the editable tier, protecting its most recent exchange.
Player claims and earlier model speech are never promoted to verified world facts.
NPCs can know authored clues (key location and gate requirements); the narrator receives
only public scene facts. A model reply cannot move the player, set prices or transfer
items. **Generated prose can still contradict facts**: local tests include invented tonic
effects and inconsistent seal permission. The game never treats that prose as authority.

### Additional example-host operations

These extend the existing JSON-lines host. Do not expose its state, diagnostics or memory
management indiscriminately as model tools. Optional fields are shown with `?`.

| op | args | Result |
|---|---|---|
| workflow | `{size,task,identity}` | Size/task profile named `dungeon`; no model configuration applied |
| interpret | `{input,context,guidance?,size?,resolve_slots?,resolve_ambiguity?,target_hint?,expected_revision?}` | Request, budget, legal named `offer`, revision, optional resolution metadata |
| speech_prepare | `{speaker,input,context,guidance?,size?}` | One-use ticket ID, speaker, request, budget/dialogue receipts |
| speech_finish | `{ticket,text}` | Validated speech/tone/source IDs; world unchanged |
| speech_state | `{speaker}` | Current accessible scope's tone, history and player quotes |
| speech_forget | `{speaker}` | Clears that scope, including its pending ticket |

`guidance` is `standard` (host API default), `grounded` or `focused`; `size` defaults to
`3`. Interpretation defaults to the legacy unfiltered control; the focused interactive
client explicitly enables both slot and ambiguity resolution. Resolution reasons include
`phrase`, `recent_action`, `explicit_target`, `ambiguous`, `unresolved_reference`,
`type_mismatch` and `unmatched`. A target hint requires `resolve_ambiguity:true` and an
`expected_revision`; neither hint nor revision may be supplied alone. Unavailable hints
and stale revisions fail. Current user text must be nonempty and at most 2048 bytes.

`speaker` is merchant, keeper or narrator. NPC access requires its current room; terminal
runs reject conversations. Utterances and speech are nonempty single lines, at most 240
Unicode characters. At most 256 tickets are prepared per session. Preparing another
ticket replaces the pending one; successful finish consumes it. A failed render or reply
does not commit social changes. Accepted game actions make earlier tickets stale.

The finish `text` is strict JSON, at most 4096 bytes, exactly
`{speech,tone,fact_ids,memory_ids}`. Tone is neutral/warm/wary; at most eight distinct
supplied fact IDs and two distinct recalled memory IDs are accepted. Extra fields,
duplicate JSON keys and unavailable citations fail. Focused generation instead produces
`{speech}`; its client wraps neutral tone and empty citation arrays before finish.
Citation checks establish source availability only, not prose entailment.

`--social-replay FILE|-` accepts `{seed,operations}` with at most 128 operations and
262,144 bytes. Operations are `act`, `prepare`, `finish`, `state`, `forget`; args match
the corresponding social/rules reference functions. Output records accepted/rejected
operations and final public world. Thirteen separate social fixture sets and 43 independent
Python/Rust workflows cover 507 accepted and 11 rejected operations; 20,000 added social
text mutations check atomic failure. Another 10,000 entity-text mutations never invent
an action or target. Offline entity parity checks cover 360 workflows across six seeds;
seven Python front-end tests use real host rules with fake generation. Core formats keep
all previous expected outputs. These are rule/validation tests, not model generalization.


### Purchase requests inside dialogue

Purchase dialogue now uses a local offer handler before speech generation. With seed 7,
“i'd like to buy the tonic” yields “The tonic costs 9 coins. You have 5, so you need 4
more.” No purchase is pending. Asking to buy hinge oil quotes 3 coins and asks for yes/no;
“yes” buys it exactly once and reports 2 coins remaining. “No” cancels. Changing topic,
`/back`, `/forget` or a stale world revision prevents a later unintended purchase. Free-form
purchase requests while exploring the workshop use the same flow. Explicit listed
commands retain their deterministic behavior. This example sells one unit per item.

The example host's `trade` operation takes `{speaker,input}` and requires live speaker
access; `trade_cancel` takes `{}`. A handled result contains `reply`, optionally `offer`
or accepted `result`; unhandled text returns `{handled:false}` for normal AI dialogue.
Generated merchant speech never creates an offer or approves payment. Local trade
receipts are shown directly, separately from model dialogue memory. Current world facts
reach the next model request. General English intent coverage remains limited.

Memory, reply validation, entity matching and confirmation now use the reusable
[core conversation APIs](../../docs/CONVERSATION.md). Prices, item visibility, action
words and game effects remain example policies. All old social fixtures and frozen model
cohorts still audit unchanged. A separate independent trade reference verifies 66
workflows / 144 trade requests across six seeds with zero model calls; 11 front-end
regressions and a real launcher smoke cover the reported loop. New API spend $0.


## Authored world file and factual dialogue

AI conversation now defaults to `--dialogue sources`. Keep using your installed local
model; no new model is required. Start with the editable file:

```sh
cargo run --example dungeon -- --ai --size 3 --seed 7 --world examples/dungeon/world.json
```

Edit the merchant/keeper backgrounds, stable lore IDs, topics, tags, `known_by` audiences
and `requires` knowledge grants. The file loads once; restart to apply edits. Omitting
`--world` uses the embedded example. Lore cannot set coins, transfer items or waive rules.
Only authorized relevant records reach a character. The keeper's vault inscription also
requires the host's `vault_open` grant. See [LORE](../../docs/LORE.md) for exact format/bounds.

Factual questions select a supplied source ID or an explicit unknown; the host renders
the answer. Player disclosures use exact quotes and attribution, while current ownership,
prices and balance come from the engine. This prevents added prose within that factual
path, but a wrong valid source can still be selected. Missing player attributes and
unobserved outdoor weather have unknown responses. An unrelated disclosure must not be
filled using an NPC biography or a name quote.

Open-ended conversation uses selected authored character lore alongside current facts;
generated prose remains unverified. `--dialogue legacy` restores the earlier control
while retaining local trade confirmation. New factual answers may be up to 512 characters
to keep a complete 240-character player quote; legacy generated speech remains 240.
The English question policy is deliberately bounded and can miss paraphrases or need
clarification. [Measured successes and failures](../../docs/LORE_EVAL.md) include twenty
new questions and eight additional questions, with all raw results retained.

Host additions: `knowledge_prepare` takes `{speaker,input,context,size,choices_limit?}` and returns a
one-use factual offer or dialogue fallback; send only its `offer.request` to the model.
`knowledge_finish` takes `{ticket,text}` with strict `{answer:id}` model text and rechecks
current scope/revision/source before returning `{speaker,speech,sources,status}`.
`lore_speech_prepare` accepts the existing speech-prepare args and adds authorized authored
lore to free-form dialogue. Existing host operation outputs remain unchanged.

## Dialogue fixes under development for v0.3.1a

The player prompt is `you>`. Unambiguous identity/stock/scene/topic questions and literal
letter counts resolve locally; explicit available commands in conversation use real
engine receipts. Attributed name recall says “You told me your name is …”. General
speech remains experimental; additional free-form phrasing passes did not establish
factual safety in the retained diagnostics.

The example clock starts at day 1, 08:00 and advances ten minutes per accepted action;
conversation does not advance it. Internal time is not automatically NPC knowledge.
The merchant has no clock; the keeper estimates a schedule phase. Authored `timekeeping`
lore can explicitly grant `clock`, `watch`, `schedule` or `none` access (structured v2:
`value:{"mode":"schedule"}`). Reported time remains an attributed, possibly stale quote.
The `world` host operation reads game time; do not expose it wholesale to characters.
`location` returns the current redacted scene. Neither underground NPC observes outdoor
weather or the sun. See [character perception](../../docs/CHARACTERS.md) for reusable
`look_around`, visibility, clocks and reports in other games, and
[measurements](../../docs/CONVERSATION_PATCH_EVAL.md) for test scope and failed pilots.

## Optional routine and sensor demonstration

Run `cargo run --example dungeon -- --routines-demo` without a model. This scripted
**surface annex** demonstration adds no sunlight to the normal underground game. It shows
a customer interrupting the merchant, resumption/postponement, blocked travel, actual sun
through an annex window, a later indoor view with no visible sun, remembered uncertainty
and expiry to unknown. Edit `routine.json` and `routine-config.json`, or pass
`--routine-file FILE --routine-config FILE`. A real host supplies these sensor/event
confirmations and executes movement/work. [Library integration](../../docs/ROUTINES.md).


## Explicit conversation actions and optional count reuse

Conversation shortcuts now require a whole explicit command/alias. Negations, conditions,
quoted commands, compound actions and unsupported wording ask for clarification instead
of matching an item anywhere in the sentence. Natural world-changing interpretations
outside this shortcut still require confirmation.

`cargo run --example dungeon -- --ai --count-cache 8` optionally retains up to eight
verified exact-request token counts. Default 0 keeps the existing preflight flow. Changing
messages, tool results, corrections, grammar, output/context or model identity recounts;
responses are never cached. Whole-turn allowance, pinned model settings and returned
counts are checked. This saves calls for exact repeats, not general dynamic prompts.
[Measured results and limits](../../docs/ADVERSARIAL_EVAL.md).

## Optional dynamic-character demonstration

Run `cargo run --example dungeon -- --life-demo --size 3` for a deterministic merchant
session. `--size 70 --life-tools` expands bounded recall/exchanges/traits and explicitly
selects optional life tools, without loading any model. The scene includes a customer
interruption, partial hearing, preference, opinion change, name correction, postponed
promise, save/load reunion and actual/blocked/remembered sun fixtures. Speech is authored
for the demo; generated prose quality is separately experimental.

Edit `../characters/setting.json`, `../characters/merchant-life.json` and
`life-config.json`, then rebuild, or load those JSON values through the persistent host
life configuration API. Default library features/tools remain off. Player reports and
corrections are attributed; author edits use a privileged revisioned operation. Engine
execution, receipt confirmation, visibility, calendars and storage authentication stay
in host code. [Smallest integration and bindings](../../docs/LIFE.md).

For human play, `--ai --dialogue natural-scaled --size 3` opts into the new tiered natural
renderer. Small unknown answers omit distractor lore; dialogue hints govern actual quote
recall and whole exchanges. Earlier `natural`, `sources` and `legacy` modes retain their
recorded behavior. Valid JSON still permits factual invention; source-selected answers
remain the default. No voice service or extra model is required by any offline example.
