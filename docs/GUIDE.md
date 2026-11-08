# Getting more from less

Practical rules for game context, in order of impact. Each says whether it was measured
with the [eval script](../eval/query/model_eval.py) (three models, Llama 3.2 3B to Llama 3.3 70B)
or is standard practice that was not.

## 1. Send only the rows that matter

The cheapest token is one you never send, and a model cannot confuse a row it never sees.
Let the game choose: the 10 nearest NPCs, the 5 most valuable items, catalog search
results instead of a whole registry.

```json
{"name": "nearby_npcs", "pack": "table", "file": "npcs.json",
 "sort_by": "distance", "limit": 10, "fields": ["name", "role", "mood", "trust"], "round": {"trust": 1}}
```

`sort_by` runs before `fields`, so you can sort by a column you do not send. *Standard
practice; the savings scale with how much you leave out.*

When the game does not know exactly which rows matter, `relevant_to` keeps the rows that
best match the words of some text: the player's message, the current event, or the
question. Rows matching more words win, so "any rare weapons?" keeps rows that are rare
*and* weapons; if nothing matches, the whole table is sent. *Measured on all 53 eval
questions with the question as the text: −26% input tokens, identical accuracy on every
question type, and the answer stayed in the table every time.*

For chat-driven turns where a table only matters when the message mentions it,
`"relevant_fallback": "none"` (`.relevant_only(text)` in Rust) sends no rows when nothing
matches. *Measured on the two smaller models: another 59% fewer input tokens, same accuracy
except questions about a table as a whole ("who has the highest trust?"), which then see
nothing: 2/6 → 0/6.* Those questions belong in code anyway: `sort_by` + `limit: 1` answers
"highest trust" exactly, while models given every row got it right 2 times in 6.

Catalogs take `relevant_to` too: send the entries that best match the player's message
instead of a keyword search. Table and catalog relevance split ID segments at `: . / -`
and then `_`, while preserving full cells/segments. Thus `scroll` and `salt_scroll` find
`grove:salt_scroll`; `gate` finds `quest.gate.open`. Unicode lowercasing is retained;
substring fragments do not match. Offline lexical regressions improved 2/10 → 10/10,
with four additional catalog parity cases. No embedding, fuzzy search or semantic
accuracy claim is implied. Existing no-match fallback and ranking rules still apply.

Round numbers to the precision decisions need. *Measured: rounding two number columns
from two decimals to one saved 5% of the whole packet; each dropped digit costs about one
token per number. Rounding to whole numbers saved 13%, but made a 0–1 trust score useless,
so choose per column.*

Long histories are the same problem over time. Keep events in a `Memory` and send the few
it recalls for this turn instead of the whole log; let the model add to it with the
`remember` tool. *Measured: 10 questions about a 60-event history, 8 recalled events vs
all 60: 84% fewer input tokens, 20/20 correct both ways on the two smaller models.*

## 2. Use tables, grouped by what the game filters on

Header-plus-rows tables cut input tokens by about a third against compact JSON at the same
accuracy. Grouping by the column most questions filter on (`group_by`) saved more and
answered better. *Measured: −33% tokens for tables, −38% grouped; grouped 140/159 correct
vs 133/159 for JSON; filters on the grouped column 18/18 vs 14/18.*

If questions filter on two columns, group by both: `group_by: ["kind", "rarity"]` nests
rarity groups inside kind groups. *Measured on the two smaller models: "every rare weapon"
questions 7/12 → 12/12, rarity lookups 7/8 → 8/8, 2–3% fewer tokens.*

Writing tables as plain comma-separated text instead of JSON arrays saved nothing:
tokenizers already treat `","` as one token. *Measured: 0 to +1% tokens.*

## 3. Describe actions as signatures

Action schemas are often the largest part of a packet. `signatures` writes them as short
lines (`power: integer 1..5; Effort level`) and describes arguments shared between actions
once. Its two header lines are deliberately short; longer explanations measured no better. Use `schema` only for tools that need nested objects or arrays. *Measured: 12–14%
fewer tokens than packed JSON Schema on all three models; 141/159 correct overall vs 140
for grouped tables with JSON Schema; actions 30/30.*

## 4. Never replace values with lookup indexes

Replacing repeated strings with indexes into a symbol list saved no tokens, because common
words are already one token, and models answered with the index instead of the value.
gamectx does not do this. *Measured: 102 → 74 of 108 correct.*

## 5. Do filtering in code, or let the model think first

Asking a model to filter long lists is unreliable in every format. "Items with count at
least 15" was right 1 time in 27 without help.

- **Best:** compute the answer in game code and send the result. `where` makes the common
  cases one line: `{"count": {">=": 15}}`, or `.filter("count", ">=", 15)` in Rust, and
  `derive` adds computed columns from [formulas](FORMAT.md#formulas) that filters and sorts
  can use.
- **Never let the model do arithmetic itself.** *Measured: asked 8 sum, average and
  difference questions over the game data, the two smaller models got 0 of 16 right.
  Replying with an expression that gamectx computed (`calc`) got 8 of 16; every remaining
  miss picked the wrong rows, which `where` and `derive` fix in code.* Give the model the
  `calc` and `define_formula` tools (`Formulas::tools`) for arithmetic it decides on itself.
- **Otherwise:** ask for a brief list of candidate rows before the final JSON. *Measured on
  18 filtering questions: 37/54 → 46/54 correct, numeric filters 1/9 → 6/9, for about 60
  extra output tokens per answer.*
- **When the model chooses the filter:** give it `query::tool()` and execute the `sql`
  argument with `query::run()`. *Measured on 29 questions per small model: 33/58 direct,
  53/58 with SQL; numeric filters 0/6 → 6/6, whole-table questions 2/6 → 6/6, arithmetic
  1/16 → 12/16. A separate run with the tool packed as a signature scored 52/58.*
  Those historical runs still sent the full tables. Only expose tables the model may
  read, send errors back for correction, and budget the result.
- Sorting rows or adding row counts did not measurably help on its own (37 → 37 and 39
  of 54), so gamectx does not add counts and does not claim `sort_by` improves accuracy.

For a query workflow, send `query::describe(&tables)` in a per-turn variable instead of
the rows. It includes column types and complete small categorical domains. Define meanings
explicitly, such as `unit_weight` and host-derived `stack_weight = count * unit_weight`.
Use `query::Session` for one correction after an invalid request; valid SQL with the wrong
meaning ends the session, so retrying cannot repair ambiguous field semantics.

*Measured with frozen prompts on three held-out states (24/48/96 inventory rows and
10/20/40 NPCs), 29 questions per state and small model: full tables 160/174 first attempt,
171/174 after correction; schemas 169/174 first, 172/174 after correction. Returning the
result to the model preserved those scores. Across the complete exchanges, average input
fell from 4,898.632 to 1,615.874 tokens (67.0% less); completed-exchange cost fell from
$0.107952 to $0.038804 (64.1% less).* The old one-state/full-data numbers above remain
historical. These newer tests still share one synthetic game family. Mistral sometimes
persists with unsupported top-N subqueries or interprets rarity `unique` as count = 1.

## 6. Keep stable sections identical every turn

Providers that cache prompt prefixes only reuse bytes that match exactly. gamectx
serializes deterministically and puts stable sections first; keep timestamps, turn
counters and anything else that changes out of them. If a budget may drop a stable
section, the cached prefix changes that turn, so give stable sections high priority or
mark them `required`. *Observed: the 70B model reported cached input tokens on 306
of its 324 requests across all runs.*

## 7. Let the game validate every action

Packed context tells the model what exists; it does not make the model obey it. Check
every proposed action in code, and use your provider's structured output (JSON schema)
mode where available so replies are always parseable. *Measured: action choice was 29–30
of 30 in every format, but production games should not rely on that.*

Packed `schema` sections are for actions described in the prompt. When you use a provider's
native tool calling, pass the tools in its own format; gamectx can still pack everything else.

## 8. Measure your own game

The original packing benchmarks use one state and three models; query workflows add
three held-out states on the two smaller models. Your data and models will differ.

- `gamectx calibrate` learns your provider's real tokens-per-byte ratio from reported usage.
- The eval script runs your own questions against any OpenAI-compatible API with a hard
  spending cap. Change `build_data` and `build_questions` to match your game.

## 9. Learn small reaction preferences locally

`brain::Brain` is an engine-independent utility network. Choose meaningful normalized
inputs and reaction names in your host, then bind them to real observations and currently
legal reactions. It can serve an NPC, player-specific preference, faction, encounter
director or other owner selected by the game. Use its compact prediction as a `Turn::var`
when the model needs it; routine local inference needs no model request.

The host supplies labeled examples and separate validation states. Training happens on
a candidate copy, with work limits, pinned parameters and bounded displacement from the
authored baseline. Only lower supplied-label validation MSE is retained; rejection is
atomic. Save learning with `to_value`, restore it with validation, and undo recent updates
without freeing replayed IDs. Schedule training explicitly rather than training on every tick.
Missing inputs remain unknown, and ties/unknown eligible scores abstain for a host fallback.
The normal game executor remains responsible for legal actions and changed-world checks.

*Offline measurement on four synthetic utility tasks, 32 training and 32 validation
examples each, fresh test seeds 707/808/909: random initialization 545/1,536 correct
reactions → trained 1,421/1,536. The nonlinear switch task scored 306/384, so the result
is not uniformly perfect. Changed trading preferences improved from 264/384 to 384/384
on fresh seeds 1001/1002/1003. Saved trained states were 1,374–1,762 bytes. No API calls.*

These measure agreement with reference labels, not human enjoyment, causal reward learning
or performance against an LLM. A lower validation loss can still miss important reaction
boundaries. Keep independent gameplay tests and host metrics, and use formulas/queries
for exact calculations. Host game logic supplies feature normalization, freshness, label
meaning, permissions, storage and rollout choices. The network does not create factual
memory from proposed lessons or from its own predictions.

When reaction choice matters, use `train_select` to compare a short list of learning
rates. It selects by correct validation choices, then MSE, and retains one candidate
only if MSE improves and choices do not regress. Supply an explicit aggregate work
budget, at most 2,000,000 units; individual trials retain the 500,000-unit bound.

*Fresh paired test seeds 2001/2002/2003: a fixed rate scored 1,436/1,536, selected rates
1,487/1,536. Nonlinear switch improved 315/384 → 359/384. Both used 32 training samples
and 128 validation samples per task; rates 0.03/0.1/0.3 were selected only on validation
data. Selection used about 3× estimated training work; runtime inference is unchanged.*
This is a different cohort from the original 1,421/1,536 benchmark above. Its utility
targets and initializer are still synthetic; host eligibility masks and thresholds
need their own evaluation, and repeatedly tuning against validation data can overfit.

For startup and tick-loop costs, load saved learning once using `restore_json` and keep
the compiled `Brain`. Clone a preloaded template for independent actors instead of
restoring the same JSON repeatedly. Each clone retains its own learned parameters,
revision, history and consumed IDs. The [preload example](../examples/runtime/preloaded-brains.rs)
demonstrates two owners and undo without changing the other owner.

Use `CachedBrain` when normalized observations repeat. Capacity is 0–256 FIFO entries;
start with a small capacity and inspect `stats()` for actual reuse. Keys include all
normalized observations, eligible reactions and thresholds. Use `brain_mut()` for
training/undo/replacement; it clears predictions before any mutation. Persist
`brain().to_value()` as usual; predictions are ephemeral. The host must refresh input
observations and action eligibility even on cache hits.

*Offline release microbenchmark, medians of five trials: 20,000 predictions took
15.939–19.223 ms cached versus 29.161–36.524 ms uncached across two network sizes and
two reuse patterns (1.7–2.0× faster). Cycling 256 states through 32 slots produced zero
hits and was 1.8–1.85× slower. Cloning 1,000 compiled templates took 1.412/8.952 ms
versus 24.238/142.615 ms restoring saved JSON for small/large networks. These are local
timings, not guarantees; [measurement details](../eval/README.md#brain-preloading-and-prediction-caching).
No API calls.*

## 10. Bound frame work and reuse context

Preload/clone a brain at startup, bind inputs once in declared order and keep one
`Prediction` per worker. `run_into` accepts integer millionths and an eligibility bit
mask; all success-path storage is fixed-size. Validate host normalization and mask
mapping when binding a different graph. Choice indices refer to that graph's output
order. Convert scores to JSON only when exposing them outside native code.

Register actor jobs with `schedule::Scheduler`, giving each a cost, minimum interval,
maximum refresh age and initial phase. Notify observation, brain revision, eligibility
and threshold changes; urgent events bypass the minimum interval while staying inside
the per-frame budget. Execute reservations or re-mark failures dirty. A budget cannot
guarantee refresh deadlines under overload, and estimated costs need host profiling.

Create a `TrainingJob` outside the critical frame path, advance it with an explicit
work budget, and finish after `is_finished()`. Allow at least `next_steps()` units per
slice (one complete sample). Dropping cancels without touching learning. Finish rejects
a brain whose complete starting state changed. Advance other jobs or keep using the
current policy while a candidate trains; scheduling training separately from inference
is a host choice. Allocation/copying at setup/transitions/commit is bounded, not timed
by work units. Keep latency measurements for your largest actual graphs and histories.

For host-policy selection, put eligibility and score/margin thresholds on each validation
row and use `train_select_host` or its resumable counterpart. Compare desired and predicted
choices under the same mask/thresholds. Do not tune validation sets against the test set.
The fresh synthetic masked/thresholded cohort scored 1,466/1,536 for both old and new
selection: appropriate metric coverage, not a measured accuracy improvement.

`Context` prepares registered sections once. Use `PreparedSection` for repeated lower-level
assembly, replacing a prepared section when its data changes. The byte-budget path counts
exact encoded sizes and only serializes the selected packet. Generic fitting predicates
still receive every complete candidate; calibrated token estimates retain their behavior.

For a stateful host connection, use `sync::Publisher` / `Replica` with an explicit stream.
Send the receiver's actual revision as ACK, and omit ACK after a reset/missing baseline
to obtain a full snapshot. Replicas atomically reject bad streams/revisions/removals/order
and preserve exact numbers. Never assume a sent message arrived. The ordinary model
request contains reconstructed context; whether a local model retains a useful prompt
prefix is a separate integration measurement. An acknowledged delta is transfer compression,
not a guarantee of fewer inference tokens or smaller retained context.

*Local offline results: 20,000 native predictions took 1,968,370/6,171,925 ns versus JSON
26,030,769/32,503,158 ns (small/large), with zero native allocations. In 500 pressured
packet builds, the old algorithm took 146,774,066 ns, new one-shot assembly 19,042,995 ns
and reused prepared sections 1,834,731 ns. Scheduling reserved 512 versus 15,360 evaluations
in a 64-actor, 240-frame trace, max eight/frame. A 120-turn context trace transferred
19,089 versus 489,490 bytes with exact reconstruction. These are synthetic workloads;
[methodology and limits](../eval/README.md#frame-work-and-context-reuse) accompany the numbers.
API spend $0; actual local-model KV-cache savings remain unmeasured.*

Run the [combined example](../examples/runtime/runtime-loop.rs) with `cargo run --example runtime-loop`.
It includes scheduled native predictions, sliced selection, prepared context, explicit ACKs
and host-policy validation. It completes 40 frames with nine evaluations and one retained
revision; a further host-metric update is rejected when its validation loss cannot improve.

## 11. Keep input meaning and remembered claims explicit

Use a `FeatureSchema` when an engine binds physical measurements to brain inputs.
Give it a stable ID and revision, declare units/ranges and missing/outside policies,
and save its complete definition with the learned state. `BoundBrain` compares the
expected schema on restore and checks graph order on each bound call. The host still
has to measure the units it declares. The [integration example](../examples/runtime/integration-tools.rs)
normalizes named physical values, stabilizes a reaction, captures a replay and supplies
query meanings. Run `cargo run --example integration-tools`.

Use `Brain::diagnose` for exact abstention conditions and a bounded `Recorder` for
observations, masks, thresholds, labels and brain snapshot changes. Replay tests what
the policy would output on recorded inputs; changing policy may change future world
states, which requires engine evaluation. Recorded p50/p95 times summarize host-supplied
elapsed values, not replay performance. Keep the binding envelope alongside normalized
traces. `gamectx replay examples/fixtures/brain/trace.json` demonstrates all abstention reasons.

Enable `StableBrain` only when fewer reaction switches help the game. It preserves the
raw prediction separately and immediately honors urgency, eligibility and abstention.
On three fresh 240-frame noisy trajectories it reduced 355 switches to 15 with zero
missed urgent switches or ineligible choices, while giving up 2,281,674 utility
millionths in total. Check utility loss and response delay, not only switch counts.

Add `query::describe_with` meanings to ambiguous field names, with actual units and
categorical literal explanations. Native validation catches metadata that disagrees
with the tables; it does not prove that a model interpreted the meaning correctly.
Three fresh 24-row states scored 36/60 → 45/60 executed queries with metadata on the
two smaller models. Token-only input rose 205→365 / 190→352; this is an accuracy/token
tradeoff on a targeted dataset. Token check $0.0001380125, accuracy run $0.0051951,
combined $0.0053331125. [Reproduction and limitations](../eval/README.md#bindings-semantics-and-evidence).

For freshness, `Journal` tracks entity/key/source channels and permanent supersession.
A model proposal cannot shadow an active observation; an expired replacement does not
revive an older observation. Use observed-source insertion only from trusted engine
code, and `apply_proposal` for untrusted proposed memories. `forget` removes individual
facts; source labels remain explicit when proposals are deliberately recalled.

For cited reasoning, use `EvidenceMemory`: record native observations with increasing
event tokens, then create a small filtered snapshot. Keep the sealed host object while
sending only its payload to the model. Apply a returned hypothesis using that snapshot
and current time/denied-privacy labels. Changes since the request, missing/expired sources,
unsupplied citations and another store's snapshot are rejected atomically.

Support/refutation links are proposals about evidence, never new observations. A
hypothesis stays tentative or contested even at the maximum 0.95 declared confidence.
The same evidence cannot renew its lifetime or increase confidence; source expiry caps
its lifetime too. The reference test keeps two observations, confidence 0.6 and expiry
30 after a repeated higher-confidence/longer-TTL request. This tests integrity, not
whether the statement is true or logically entailed by its citations.

Use `contradict` for invalid native evidence, and call `forget_privacy` when sources
must be erased; dependent hypotheses disappear too. Filtered snapshots take effect
immediately, while physical removal requires forgetting. Clock identity and subject are
checked on restore. Define clock progression in the engine, especially during pauses
or disconnected-player time. The library does not own archival files, consent policy,
active-player timers or permissions. [The tested evidence example](../examples/knowledge/evidence-memory.rs)
runs with `cargo run --example evidence-memory`.

Use `facts::FactRegistry` for explicit engine/user assertions that should survive event
recall: configuration rules, preferences and validated state. Register a scope, key and
typed JSON value, retaining its returned `FactRef`. Pin rules that require deliberate
changes. User-owned assertions can only be changed with user authority; a pin must first
be explicitly removed. Supply the current revision when replacing any existing key.
The authority enum records a host decision; it does not authenticate a caller.

Attach source references to host-validated derived facts. A changed, missing, private or
expired source makes descendants unavailable until the host recomputes them. Unpinning
also advances the source revision. Ordinary forgetting leaves invalid descendant payloads
in saved state; privacy erasure physically removes matching data and its descendants.
Capacity is explicit and full registration fails instead of evicting a rule silently.

Use `check` on model-proposed typed claims, then treat `supported`, `conflicting` and
`unknown` according to the game policy. Missing/private/expired/stale facts are unknown,
not false. Decimal comparisons are exact, object order is ignored and array order/types
matter. A matching registered assertion still depends on the quality of its source;
free-form statements require host interpretation and evidence checking.

`Turn::facts` adds a filtered, byte-budgeted table with a dropped count. The offline
128-fact fixture retains eight in 927 bytes under a 1,024-byte cap and reports 120 dropped;
direct lookup can still retrieve those current facts. This is a packet-size/integrity
check, not a model-accuracy result. [The tested example](../examples/knowledge/registered-facts.rs)
runs with `cargo run --example registered-facts`. Use `Journal` for observations and
`EvidenceMemory` for tentative reasoning; neither automatically promotes ideas into
registered facts. The host owns archive I/O, validation and any model-facing tool routing.

## Reuse and checked runtime work

The [fresh alpha.3 model check](evidence/MODEL_TEST_2026_11_06.md) compares full JSON, packed tables
and schema + query across six states/three schema families on the two smaller models.
Scores were 27/96, 27/96 and 91/96; average input per completed exchange was
1,786.792, 1,252.125 and 916.490, including query/correction/final-answer calls. Packing
alone keeps the aggregate score while reducing tokens; native queries handle arithmetic
and row selection more reliably in this cohort. Valid queries can still add unintended
limits/filters, and these measurements do not establish live-game accuracy.

Start with the [tested synthetic host example](../examples/runtime/runtime-upgrades.rs):
`cargo run --example runtime-upgrades`. It makes no real requests or game actions.

Build `RecallIndex` from records already filtered by subject, time and consent. Keep the
original IDs/provenance in the host, and rebuild the index after edits or forgetting.
Enable suffix normalization/aliases explicitly and test false matches as well as misses.
No match now returns nothing, so make any fallback policy explicit. The selective fixture
uses 100 posting visits for 100 queries/1,000 records versus 100,000 full-scan record
visits; construction/storage costs are extra. Two simple alias/normalization matches
are integrity examples, not evidence of broad semantic accuracy.

Use a host-validated `Playbook` when a situation has a known procedure. Store exact fact
references and typed required values with each recipe. On reuse, the library checks
current facts and asks your callback to validate the action and arguments under current
schemas, resources and permissions. The returned draft still needs execution-time checks.
One repeated fixture matched 100/100 locally; no measured live API savings are claimed.
Changed source revisions invalidate a procedure even if its value happens to stay equal.

For learning, call `OutcomeTracker::begin` when considering a trial, `executed` only
after native success, and `exposed` only after an actual observation opportunity. Supply
a measured score through `resolve`, then consume `take_label` once, joining its ID to
your retained input/action record. Send it through existing holdout/mask/pin/replay guards
before accepting any weight change. If the actor never had a chance to notice the action,
leave it unknown. Offline checks produce 100 labels, zero unexposed labels and zero
replayed labels; they do not establish causality or gameplay quality.

Use `TemporalWindow` for bounded native counts, rates, numeric changes or event sequences.
Each source event needs a distinct monotonic ID and original time. Mark observation gaps
unavailable; doing so clears old data and returns unknown instead of zero. Reset coverage
on privacy/subject/world changes and erasure. Retain source watermarks durably if your
engine replays events across restarts. These helpers create features; they do not infer
personality, motives or certain habits from repeated events.

Use `BrainBundle` when perception is the same for several candidate reactions. Prepare
once for one actor/frame, then run the decision module for each candidate with imported
slots empty. Original unknowns remain unknown. New frames and compatible module updates
invalidate prior sealed results. Train cloned perception/decision modules independently
using existing validation, then replace one; the other module's pins/history stay intact.
Three fresh cohorts produced 3,600 predictions identical to duplicate evaluation. For
12 candidates on each of 100 frames, module evaluations fall 2,400→1,300. Paired release
medians for 12,000 candidates: 729,906→631,514 ns (zero hidden nodes/module),
3,341,042→2,014,761 ns (16 hidden). Benchmark your own network sizes/change rates.

For an asynchronous model reply, mint a `RequestGate` ticket with actor/world, schema and
permission revisions, relevant fact references and TTL. Supply independently captured
*current* context to accept, never reuse the request's context object as current state.
A stale result consumes its ticket and is discarded; a fresh one still needs your normal
native action validation. Drop/cancel outstanding work on disconnects or consent changes.
No state ticket can detect a dependency the host omitted.

For hosted requests, reserve the full worst-case cost in `SpendingLedger`, atomically
persist it, mark dispatch durably and then send. Settle reported usage afterward; each
retry gets a new reservation. Recovery conservatively charges interrupted requests at
their full estimates, and metering overruns block new paid reservations. Free/local
fallback remains possible. The library handles accounting, while your host owns pricing,
HTTP limits/concurrency, durable saves and operator controls. All seven additions and
their checks used $0 API; [the eval guide](../eval/README.md#optional-runtime-upgrades)
records methods and limits.

## Inspect query structure and reuse parsed requests

Parse a repeated host query once with `Query::parse`, then call `run` on each current
snapshot. The query stores no row snapshot or cached answer, so updates remain visible.
Execution borrows row data; it still validates every referenced column in every row,
including rows beyond LIMIT. Sorting validates all filtered rows before truncating.
Aggregates retain their original left-to-right floating-point arithmetic and null rules.
Five local fixtures measured 4.6–32.5× faster parsed execution, not model response time.

Use `Query::plan` to review a model request before executing it. For structured requests
where the host knows the required shape, `query::Shape` can reject an extra LIMIT or
filter column. Exact shape is too restrictive as a general natural-language validator:
an offline oracle-shape diagnostic rejected 27/96 recorded requests, including 22 correct
equivalents. A shape also does not check operators, values, selected columns, sort order
or permissions. Expose only authorized tables/columns and keep host action checks.

`run_bounded` checks input row count and output bytes without truncation. Its output check
happens after computation; cap host cell sizes separately if transient memory matters.
See the tested [query plan example](../examples/data/query-plans.rs) and
[format contract](FORMAT.md#query-plans-and-host-contracts).

Choose supplemental query guidance per model/workload with
`query::Guidance::parse(configured_profile)?.text()`, and append the returned text once
to the system prompt. `standard` is empty; `explicit-selection` emphasizes all matching
rows, requested filters only, and the native top-N aggregate syntax. Keep it across
correction/final turns; do not repeatedly append it. Six fresh synthetic states measured
3B 46/48→48/48, 24B 47/48→46/48 and 22.0% higher complete-exchange input. It is opt-in;
model size alone does not choose a profile. See the tested query-plans example and
[paired report](evidence/QUERY_OPTIMIZATION_2026_11_06.md).

## Shared registration and integration tools (v0.2.0a)

Use the optional [integration kit](guides/KIT.md) when the same data definitions need to drive
packing, query meanings, validation and brain feature units. Start with the tested
[registered project example](../examples/kit/registered-project.rs), or generate the CLI
starter with `gamectx init <new-directory>` and run its `check.py`.

Register immutable dataset/tool definitions and named recipes, then supply current,
actor-visible snapshots. `Project::build` returns a model packet and a separate host-only
inspection. Recipes choose rows versus schema, columns, filters, sorting, limits, tools,
extra section priorities and a byte budget. Existing memory/fact APIs produce values you
can supply as recipe sections; sanitize those values in the host. Tool callbacks still
check current world permissions after argument validation.

`doctor`, `inspect`, persistent `serve`, native and language adapters, and offline recorded
run evaluation are described in [KIT](guides/KIT.md). The evaluator requires complete candidate
cohorts and independent expected answers; missing usage stays null. On its scripted three
states the correct candidate scores 3/3; an intentionally flawed one scores 2/3 with one
invalid query and one invalid call. These are evaluator checks, not hosted-model results.
The implementation has 144 byte-identical Python/Rust workflows and $0 new API spend.

## Local models and complete requests (v0.2.0a)

Use [LOCAL](guides/LOCAL.md) and the tested [local turn example](../examples/packet/local-turn.rs).
`RequestBudget` accounts for the full rendered request through the host's actual tokenizer,
reserving generation and safety headroom. Packet byte/ratio estimates alone omit external
messages and hidden native tool/template framing. Recheck every correction/tool reply.

`Scene` preserves supplied required facts, approved outcome, current input and protected
whole exchanges; drop optional lore first and fail when the protected minimum cannot fit.
`Choices` makes the model choose from a tiny host-resolved action/target set, with local
validation and one correction before fallback/abstention. Existing SQL/formulas compute
filters and totals; the model does not need to perform that arithmetic.

Profiles are explicit and bound to complete deployment identity. Two thousand context
tokens is a starting trial, not a model capacity limit. `ollama_eval.py` measures actual
local requests, complete turns, retries and sampled memory. Token-only checks precede
accuracy. Compare held-out scenes/templates and actual hardware before changing defaults;
see [the measured report](evidence/LOCAL_MODEL_2026_11_06.md) for scope and limitations.

## Further optional optimizations (v0.3.0a)

Use the [tested optimization example](../examples/packet/optimized-turn.rs) and
[workflow guide](guides/OPTIMIZATION.md) after establishing the existing local-model pipeline.
Start with a host-declared intent boundary if a legal correction could change the task.
For common questions, register a typed query template and bind arguments as data instead
of asking a small model to write SQL. Keep arbitrary SQL available for other questions.

Select complete dependency closures for evidence, then count the complete model request.
Non-Rust hosts can use the new composition handshake with their own renderer/tokenizer;
this does not turn the existing JSON scene byte estimator into an exact tokenizer.
Reuse rules, eligible procedures and brains through the optional local-match router, with
ambiguity going to a model/clarification path and execution still controlled by the host.

Readable-choice responses reduced prompt counts in six local probes and scored 11/12
versus 8/12 in a small matched-order Qwen cohort; they remain opt-in. Do not choose that
format just from parameter count. Compare your own snapshots and phrasing with the
configuration scorer, select on tuning cases and report held-out failures honestly. That
scorer checks complete recorded cohorts and costs; it does not run models or discover the
right recipe. The detailed report separates synthetic byte, routing and model evidence.

## Test a complete game loop

[Lantern Vault](../examples/lantern-vault/README.md) is a playable six-room dungeon using the
current integration kit, typed queries, scene packets and readable decisions. Start with
human play and the deterministic replay checker, then use the optional local-only agent.
Measure completion and state changes separately from valid actions; the first local Qwen
smoke repeats legal look commands without progressing. Treat that as an observed planning
gap, rather than declaring success from schema validity. Test new seeds and question
phrasing before changing guidance or recipes. The game and all offline checks need no model.


## One model job at a time (v0.3.0a)

Use a size-and-task profile as an editable starting point, then measure the exact model,
quantization and template. Interpret the immediate player request independently of the
quest goal. Let one NPC speak from current facts and its own scoped dialogue. Query or
compute exact results locally before narration. Supply implemented item capabilities,
not just evocative labels. Reserve the full rendered request plus output and safety.

Prefer complete entity phrases over overlapping short names. Keep candidate IDs stable,
use only fresh scoped referents, and ask “which key?” when several candidates remain.
Never turn a guessed match or generated speech into inventory permission. A targeted
human selection must still pass the current revision/visibility/action checks.

The [workflow guide](guides/WORKFLOWS.md) records real failures as well as smaller packets:
adding more grounded instructions regressed one 3B intent pilot from 2/4 to 1/4; exact
host slots and clarification did better on that tiny regression set. This does not prove
universal semantic understanding. Required facts and recent required exchanges remain
protected; a size hint must never silently remove them.


## Let dialogue reach validated actions

Use the shared [conversation and confirmation APIs](guides/CONVERSATION.md) to separate
speech from a pending action. The host supplies current scope/facts and stores original
player quotes; it validates the model's reply and source IDs. A clearly requested trade
can instead return a local offer containing actual item/price/quantity terms. Human
approval must match that offer's scope, world revision, terms and current permissions
before atomic dispatch. An unaffordable request should explain the exact shortfall
locally instead of repeatedly asking for approval. An NPC sentence is not an offer.

Inventory arithmetic already belongs in local queries: `COUNT(*)` counts rows;
`SUM(count)` counts units when the host provides that field. Reuse catalog/memory tags
for retrieval. Neither conversation memory nor tags establish item ownership or consent.
The runnable [shared example](../examples/character/conversation.rs) exercises memory, confirmation
and inventory totals without a model.


### Test recall and invented memories separately

A model may correctly recall “My name is Fen, and I collect stories” after inventing a
home or claiming the player disclosed an unrelated past. Check `/memory` in the dungeon:
only original player words belong in that quote store. Assistant dialogue remains
unverified context, even when it sounds confident or uses an available citation ID.
Ask the model to distinguish exact supplied quotes, current host facts and unknown
attributes; source availability alone does not validate its explanation. `/forget`
clears conversation material and pending replies, while host-owned inventory persists.
The [reported regression audit](evidence/DUNGEON_FEEDBACK.md) preserves examples and verifies
these storage/state boundaries without claiming the prose hallucinations are fixed.


### Author world knowledge and bind factual answers to sources

Load a versioned lore file once, filter by the actual character and host knowledge grants,
then select relevant records for this question. Keep private records out of model packets
and receipts, preserve required records or fail, and count the final rendered request.
Current ownership/prices/capabilities come from the host. Matching live fields suppress
stale authored fields; an unavailable attribute needs an explicit unknown answer.

For small models, offer a short set of named factual answers and an unknown option.
Validate the returned ID and current source permission; render source text locally.
Player quote answers should retain their exact words and attribution. Open-ended character
flavor can use authored background without promoting generated dialogue into facts.
This limits invented prose in factual answers; a model can still choose the wrong valid
source. Keep question interpretation, source validity and world changes as separate scores.
See the [tested API example](../examples/knowledge/lore.rs), [contracts](guides/LORE.md) and
[retained model results](evidence/LORE_EVAL.md).


## Give conversation optional information tools

Register `info::Notes::tools()` with your runner and dispatch bounded argument text to
`Notes::call`. Bind one audience scope and original evidence outside model arguments.
For ownership, distinguish a reported claim from a fresh `Inventory::lookup_call` result.
Use separate scopes for private conversations, expose only visible owners, and check a
trade's revision and host permissions before committing. Several player IDs can be owners;
no single-player constant is required. See [contracts and limits](guides/INFO.md) and the
[executable example](../examples/knowledge/info_tools.rs). Include every enabled schema and tool result
in the full-request budget; these tools do not automatically improve model accuracy.


## Let characters respond to consequences

Use `dialogue::InteractionTools` to publish witnessed changes, retain reported promises
and preferences, resolve ambiguous references, and propose exact host-enumerated offers.
Publish progress only after game validation. Bind private scopes and specific offer
recipients so one player's conversation cannot approve another player's transaction.
The [tested multiplayer-capable API example](../examples/character/conversation_tools.rs) shows the
flow without networking or model calls. [Contracts and context costs](guides/INTERACTIONS.md).

## Character perception and capacity

Use the optional [character facade](characters/CHARACTERS.md) for an actor-bound `look_around`,
location/world snapshots, existing notes/inventory tools and task-specific context.
Bind redacted views each turn; exact game time is not automatically NPC knowledge.
Clock/watch readings, approximate schedules/sun observations and attributed reports
have different provenance and precision. Keep output and safety reservations while
counting the complete provider-rendered prompt. Increasing capacity can retain more
relevant data, while explicit attention limits keep a small model on one task.

The [new measurements](evidence/CONVERSATION_PATCH_EVAL.md) distinguish local routing and byte
parity from unsuccessful live speech diagnostics. General free-form speech is still
experimental; it is not a verified paraphrase of authoritative facts.

## Add usual activities only when your game needs them

Start with the [tested optional routine example](../examples/character/routines.rs) and
[editable dungeon demo](../examples/lantern-vault/routine.json). The engine resolves condition
IDs, calendars and permissions; the local planner selects intended activities without
LLM calls. Schedule dirty NPCs through the existing work budget. Only engine receipts
confirm completion, and only actual sensor/event data creates an observation.

Enable history/time separately and specify usable sources. A walk on the plan does not
prove sunlight. A window can permit a direct sun estimate; clouds or occlusion can block
it. Preserve aging ranges, reports and contradictory readings instead of fabricating
confidence or exposing the simulation clock. Select current/next intent, required facts
and a few relevant experiences for a small model; explicitly configured richer context
still uses full rendered-request counting and protected facts. [Contracts](guides/ROUTINES.md).

For more responsive routines, use `next_wake` with your timer queue and `affected` for
planning-event dependencies; both feed the existing scheduler. Use explicit observation
selection with required source tokens when the newest experiences are unrelated to the
current task. Missing required evidence should cause a refresh or unknown, rather than
quietly substituting other memories. The [tested context example](../examples/character/routine-context.rs)
combines these paths without model calls. The measured host-copy optimization and its
synthetic-workload limits are recorded in [ROUTINES](guides/ROUTINES.md#optional-task-selection-and-responsive-wakeups).


## Validate meaning as well as legal syntax

The [new adversarial probes](evidence/ADVERSARIAL_EVAL.md) found that a legal action could still
violate a negated command, and a perfectly valid JSON reply could misread time sources.
Use a whole-command grammar for local shortcuts, clarify unsupported clauses, and keep
natural world-changing interpretations behind host confirmation. Prefer typed
`time_assessment` for exact source-policy decisions; render source readings and reports
with attribution instead of asking a model to classify them again.

For repeated identical Ollama requests, the dungeon's opt-in `--count-cache 8` can avoid
redundant preflights while preserving verified full-request counts. Tool results,
corrections, budgets or identity changes require a fresh count. The measured benefit
applies to exact repeats; dynamic game turns often miss. Shorter context did not improve
accuracy in the fresh 16-state probe, and free-form speech remains experimental.

## Opt-in character continuity

Start with `cargo run --example character-life`, then the original dungeon `--life-demo`.
Load a host-approved definition under an explicit setting/calendar and fixed audience.
Bind only actor-visible facts; ingest actual experiences with original sources. Use
`ContextCaps::from_hints(size.workload_hints(Workload::Dialogue))` or explicit bounded caps
with `Coordinator::select_context`; feed its packet into protected `TurnPlan` facts.
Count the entire provider rendering through the existing budget/count handshake, including
selected tools, template, history, results, retries and output/safety reservation. Required
facts fail overflow. Render stable system/character/scene context before changing state and
whole dialogue exchanges using `render_character_chat`.

Small dialogue hints retain one whole exchange and one or two episodes; larger hints
retain two or three exchanges and three to six episodes. These are editable policies,
not measured capability limits. Spare context does not enable tools. Register requested
`life_recall`/`life_change` only when appropriate, then expose relevant schemas and recheck
live permissions. Small integrations can appraise entirely in host code with no model.
Balanced expression is an explicit policy, not a grounding guarantee.

Validate proposed text/actions, execute only through host rules, and acknowledge actual
played/heard portions and results. A generated reply never confirms delivery or an event.
Persist consequential events during conversation; archive acknowledged originals before
eviction. Load an earlier save into a fresh instance with branch-specific fences. Host
storage authenticates IDs, transactions, replay floors and scoped erasure across archives,
snapshots, indices and external speech queues. [Contracts and host operation table](characters/LIFE.md).

For short factual speech, opt into `Life::compile_speech` version 2 rather than sending
legacy balanced policy unchanged. Use `SpeechTurn::from_hints` and host-declared known-state;
mark player attributes with explicit subject and unknown status. The compiler leaves the
actual utterance outside JSON and asks focused models for grounded short speech. Diagnose
sentence endings with `assess_sentence` before queueing; retain raw rejected output.
[Runnable example](../examples/character/character-speech.rs) and [paired results with retained failures](evidence/LIFE_SPEECH_V2_EVAL.md).

For multiple NPC roles, use an explicit `archetype::Blueprint` or compose modules with
`BlueprintBuilder`. The [party example](../examples/character/party.rs) installs approved life and
routine data, selects authoritative stock, reserves a spotlight route, and compiles
different replies for two players. Feed only host-confirmed hearing to `PartyScene`; apply
sidecar `AttitudeGates` before selection, and dispatch any exposed Life tools through
`PartyScene::call_life` so recall cannot bypass gates. Recount the actual provider request
before dispatching a one-use floor grant. [Contracts, limits and offline evidence](characters/ARCHETYPES.md).

For one-file towns, load `cast::CastPack` and explicitly accept it with `LivingWorld::new`.
The [tested example](../examples/character/living-world.rs) performs deterministic routine/rumor/
initiative work before issuing an immediate authored fallback. Keep optional model work
outside the frame callback; validate a completion and freeze before actual caption/audio
delivery. Use the world compiler/tool dispatcher so group influence participates in gates.
[Living-world contracts, retention limits and timing evidence](characters/LIVING_WORLD.md).

## Compile and repair visual assets without model calls

Use [the visual compiler](guides/VISUAL.md) to keep an authored style, silhouette, camera and
palette consistent while host-confirmed trust/emotion, active activity and perceived facts
select discrete expression/pose/environment cues. Start with `cargo run --example visual`
and edit `examples/fixtures/visual/chime-art.json`; the example runs as a test. Closed shutters and
sunlight share an authored conflict group. Unobserved/global fact text never reaches the
rendered tokens. Queue the returned job externally and display only the selected approved
fallback (or your placeholder). Submission alone does not approve an image.

Inspect a key, then apply bounded token/negative/prop/seed repairs or pin reviewed hand edits.
[Persistent binding operations](guides/KIT.md#persistent-visual-asset-sessions) use the same Rust
registry as the CLI/native ABI. The 64-workflow offline artifact measures zero model calls;
no provider, image-quality, latency or frame-rate claim is made.

## Play Lantern Vault in a local browser

`cargo run --example dungeon -- --web --cap 0` starts the opt-in standard-library host
on loopback. [The studio guide](studio/STUDIO.md) covers party controls, text/voice gameplay,
provider/tier maps, the asset debug drawer, and save/load slots. Models never execute
world actions. Interim STT only previews/interrupts; final input enters the validated
host command path. Only confirmed playback prefixes become heard memories.

Text, images, TTS and STT have independent backends; the default 0B/cached/off configuration
works without installed models. Online ceilings and a positive startup cap must be supplied
explicitly. Paid reservations survive failures/restarts and cannot roll back with a branch.
All automated studio checks use offline/fake devices and `--cap 0`; actual voice and image
quality are unmeasured. Provider configuration and the tested example are editable in
`examples/lantern-vault/web/`.

For ordinary heard dialogue, `LivingWorld::record(actor, event)` accepts a host-validated
non-retained event without reserving a rumor slot. `Life::trim_transient(keep)` preserves
retained/appraisal evidence and its complete active causal groups. Use `observe` for
consequential shareable experiences; it retains its explicit bounded-root contract.
To transform timeline entries and world events into sensory Life experiences automatically,
use the [witness bridge](characters/LIFE.md#witness-bridge-gap-1) (`character::life::witness::WitnessBridge`
or local kind `witness`), which enforces line of sight, hearing thresholds, sleep/unconscious states,
and provenance (`direct` vs `report`).
Physical embodiment, fatigue, and needs dynamics are managed via the [body and needs](characters/LIFE.md#body-and-needs-dynamics-gaps-2-and-12)
module (`character::body::BodyManager` or local kind `body`). Needs advance over ticks and are satisfied
by actions or items. Fatigue accumulates with action costs and recovers during awake rest and sleep.
Active conditions (`asleep`, `sick`, `unconscious`, `dead`) enforce physical action constraints, silence
characters during sleep or incapacity, and override daily routines with forced rest.
To resolve competing drives when needs, scheduled routines, promises, sheet goals, and emergencies
conflict, the [motive arbiter](characters/LIFE.md#motive-arbiter-gap-3) (`character::motive::MotiveArbiter`
or local kind `motive`) maps them onto a bounded 0..=100 scale, selects the winning motive deterministically,
and explains the choice.
When a motive or goal requires work, [tasks and planning](characters/LIFE.md#tasks-and-planning-gap-4)
(`character::task::TaskManager` or local kind `task`) compiles it into a tracked task with action steps,
preconditions, and expected effects. The deterministic planner solves causal prerequisite chains over the Action
Catalog; model proposals and replans are host-validated, and only confirmed world outcomes advance steps.
Character emotional states and mood dynamics are tracked via [affect dynamics](characters/LIFE.md#affect-decay-and-missing-feelings-gap-6)
(`character::affect::AffectManager` or local kind `affect`), separating background mood (valence and arousal) from targeted interpersonal attitudes (`fear`, `respect`, `gratitude`, `envy`, `admiration`, `trust`, `resentment`, etc.) and decaying emotions toward temperament baselines over time.
Skills grow through practice via [skills dynamics](characters/LIFE.md#skills-growth-with-use-gap-8)
(`character::skills::SkillsManager` or local kind `skills`), applying diminishing returns, scaling action duration, risk, and output quality, and enabling teaching between characters.
Perceptions of other minds are modeled via [theory of mind](characters/LIFE.md#theory-of-mind-gap-7)
(`character::mind::MindManager` or local kind `mind`), tracking per-other beliefs (`knows`, `wants`, `feels`) with provenance sources (`seen`, `told`, `inferred`) and witness-based knowledge deduction from timeline events.
Truthful and deceptive communication is audited via the [deception ledger](characters/LIFE.md#deception-ledger-gap-13)
(`character::deception::DeceptionLedger` or local kind `deception`), maintaining a told-ledger per listener, checking claim consistency before speech, and evaluating contradiction penalties when falsehoods are exposed.
Personal bonds, authored sheet baselines, and group affiliations are reconciled via [relationship reconciliation](characters/LIFE.md#relationship-reconciliation-gap-9)
(`character::relationship::RelationshipManager` or local kind `relationship`), enforcing documented precedence (personal attitudes > sheet relations > community standing bias > viewpoints), dispatching action outcome appraisals, and aggregating member attitudes into community relations.
The loopback studio journals up to 8,192 world operations for branch replay; this is a
bounded demonstration, not unlimited durable world storage.

Use `Life::recall_selected` for checkpoint consolidation recall with explicit current
player subjects and host-chosen required anchors (for example an unresolved promise).
Supply only the relevant anchor on focused turns: a corrected name and its original need
two episode slots; five unrelated anchors do not fit a two-slot packet. Missing required
evidence is an error, rather than a plausible but incomplete memory. Python and Node
Character adapters provide `life_recall_selected` / `lifeRecallSelected`; generic host
life operations expose the same selection in other bindings. Disk archives remain
host-owned. Never share private participant scopes merely to improve recall.

For focused service replies that need deterministic factual bounds, construct existing
`dialogue::Answers` from authorized host sentences, use `speech_schema()` as a provider
schema, and validate with `resolve_speech()`. A provider ignoring the schema still cannot
introduce a new sentence. This selects host-authored wording, so it limits expression;
use ordinary dialogue when that tradeoff is inappropriate. Source presence is not proof
that a host-authored sentence is correct. Supply current validated cards and honest
attribution for reports and claims. The Web studio exposes grounded/dialogue policies;
old configurations without `speech_mode` preserve dialogue behavior.

`character::roleplay::resolve_command` can route a small explicit English grammar without a model.
Supply only current legal pairs, visible labels and a recent focus already scoped to the
player/scene. The Web example expires focus after four ticks and clears it on player/NPC
switches, travel and branch loads. A typo or multi-item instruction asks the player to
choose a listed action; questions and negation stay dialogue. The helper never executes
its proposal. The host still applies the current game revision and rules.

Gated cast turns use `LivingWorld::compile` or `PartyScene::compile` with the cast's
current Life, blueprint and attitude gates. The legacy single-Life `compile_speech`
renderer predates party gates and has no gate input; feeding a cast definition directly
to it can expose its gated facets. After checkpoint recovery, pass the recovered Life
to the party compiler before adding bounded, host-authorized archival episodes. The
overnight campaign evaluation retains a failed renderer integration and a corrected
public-API host adapter in `eval/overnight/overnight_campaign_compile.rs`.

Optional selected recall can match `wick` against archived `wicks`; use required causal
anchors for facts that must be present rather than relying on lexical similarity.
The restored-cast paired holdout passes 12/12 required-answer checks with host-authored
sentences, versus 3/12 when generic deferral is allowed. This bounds wording rather than
improving unconstrained model reasoning; a sole allowed sentence needs zero model calls.


For the Lantern Vault preview, open **Model setup**, check local servers, choose an
installed model and **Apply changes**. Setting a tier alone does not select installed
weights. The visible activity log and reply-origin label explain whether an LLM ran or
the offline line was retained. **New game** resets current play while preserving settings,
branches, art approvals and spending; see [the Studio guide](studio/STUDIO.md).

Use **Models & Services** to choose normal/dramatic text and image profiles, audio services,
residency and predictive warming. Configure what counts as dramatic or force a scene mode.
Only installed/configured profiles are eligible; online calls remain under the launch cap.
The [manager guide](studio/MODEL_MANAGER.md) explains server ownership, art approval and limits.

Native voice integration and the Settings menu are documented in [Native voice](studio/NATIVE_VOICE.md). Engines/weights are separately installed; the host launches one bounded audio job at a time, removes temporary recordings, and stops owned process groups on cancellation.

Two-player LAN conversation keeps all processing on the host. See [LAN play](studio/LAN_PLAY.md) for the private listener, join code, guest permissions, delivery/floor rules and offline evidence.

Buffered Piper playback and microphone recovery are documented in [buffered speech](studio/BUFFERED_SPEECH.md).

Voice recording history, cancellable local replies and the floating context/execution
profilers are documented in [Studio diagnostics](studio/STUDIO_DIAGNOSTICS.md).

Use Settings → Characters in the local studio to enable editable dispositions and listener-scoped saved opinions. [Character viewpoints](characters/CHARACTER_VIEWPOINTS.md) explains complete-heard confirmation, branch persistence, bounds and model limitations. Thinking hides the previous reply until the new line is ready; Use instant reply keeps the 0B path available.

Settings → Characters → Optional world reactions enables host-approved gifts, loans and support. Only currently eligible offers appear in play. See [rules, host evidence and replay boundaries](characters/CHARACTER_REACTIONS.md); other world actions require registered engine handlers.

With personality turns enabled, try “Hello”, “How are you today?” and, after reading/hearing the reply, “Why is that?” Greetings need no LLM; mood and explanation turns are immediate and temporary and do not replace saved opinions. [Routing and evidence](studio/STUDIO_CONVERSATION.md).

With personality turns enabled, travel to a character's room to see an arrival and contextual greeting; **Approach** engages without speaking for the player. Typed `*enters the workshop*` still needs a legal route. Read/Play confirms an introduction. Settings → Characters → Greeting behavior adjusts quiet turns. [Memory, venue, party and save behavior](characters/CONTEXTUAL_GREETINGS.md).

## Give any character a configurable roleplay engine

Use `character::roleplay::engine::RoleplayEngine` when you want Lantern Vault-style
roleplay without writing it yourself. You get greetings that remember encounters, social
lines, remembered names and preferences, perception, props, stage actions and
directions, and commands. Build an `EngineConfig` (JSON or builder) and switch off the
stages you do not need. Each turn, send the player's text with the current scene and
legal choices. Execute only the `action` decisions you validate, compile a model request
for `model` outcomes, and call `delivered` with what the listener actually heard. See the
[roleplay engine guide](characters/ROLEPLAY_ENGINE.md).

Give the character depth with a **character sheet** (persona, services, goals, needs,
relationships, factions, secrets gated by trust or your own flags, opinions) and the
world with a shared **codex** (places with spots, ambience and house rules, entrances
with open/locked/hidden states, factions, timed events, quests). Send live values with
each turn: `trust`, `needs`, `quests`, `entrances` and your own `state` flags such as
`{"time":"morning"}`, which any `when` condition can test. Query legal affordances and
costs via the shared [action catalog](guides/WORLD.md#actions) (`knowledge::action_catalog`).

Pack the authored side into one **game data file** and runtime memory into one **save
file** with as many named states as you like (`slot-1`, `autosave`, a fork made before a
risky choice). `gamectx::kit::save::{GameData, SaveFile, SaveState}`; see
[saving](characters/ROLEPLAY_ENGINE.md#saving-game-data-and-saves) and the
[file format](FORMAT.md#game-data-and-save-files-v1).

## Studio reasoning and speech

See [deliberation controls](studio/STUDIO_DELIBERATION.md) for opt-in, capability-aware native reasoning, unverified notes and per-browser automatic playback. Existing formats and default outputs are retained.

## Conversation length and stage actions

See [conversation extensions](studio/STUDIO_CONVERSATION_EXTENSIONS.md) for bounded sentence tickets, longer player input, unverified optional summaries and `RoleplayActions` registration. Host validation and spending caps still apply.

## Give connected characters their own wording

In Settings → Characters, enable AI character wording; applying a text connection also
enables it. The model words source-bound facts and acknowledgements, while separate
personal/social/fiction contracts allow bounded expression. Offline or invalid generation
keeps the authored fallback. The library's `studio-expression` and `studio-wording` local
contracts preserve old defaults and share all binding transports.

`cargo run --bin gamectx -- local examples/fixtures/cast/expression-fixture.json` renders a
confirmed-perspective fictional turn; the fixture runs in the documented API tests.
Only pass host-selected facts/attitudes and actually heard history. A character's
invented tale is not an observation, and a player's disclosed name is still a claim.
See [formats, evidence and limitations](studio/STUDIO_EXPRESSION.md) and the
[short roleplay investigation](research/ROLEPLAY_RESEARCH.md).

For current NPC location and perceived dialogue, reuse `Locations` with an actor-visible
snapshot and the current revision. Do not give an NPC the player inventory to answer
where it is. The Studio now demonstrates this path, heard-seed story rotation and
retryable blocked audio; see [scene repairs](studio/SCENE_SOCIAL_REPAIR.md). For a larger
character implementation, use the [feature/gap map](research/SOCIAL_CHARACTER_GAPS.md) to
distinguish storage mechanisms from implicit memory use and motivation-driven behavior.
