# Model evaluation

## Layout

Independent Python references, offline checks, local/hosted model runners and their frozen
artifacts, grouped by topic. Run every script from the repository root with
`python3 -I eval/<topic>/<script>.py`; Python 3.12+ and, for `.mjs` checks, Node 22.

| Folder | Topic |
|---|---|
| `query/` | Packing, query tool, query workflow/guidance, calc, relevance, release model checks |
| `local/` | Local deployment contracts, Ollama runners, workflows, optimization, adversarial probes |
| `runtime/` | Reaction brains, runtime upgrades, integration kit, facts and evidence |
| `knowledge/` | Lore, information/inventory tools, conversation tools, location and world |
| `character/` | Life, continuity, initiative, routines, archetypes, parties, living world, opinions |
| `media/` | Visual compiler and asset registry |
| `kit/` | Game data and multi-state save files |
| `lantern-vault/` | The dungeon example: rules, social/knowledge/trade routing, natural speech |
| `studio/` | Studio host, roleplay features, voice, model manager, browser drivers |
| `overnight/` | Overnight deep-evaluation campaigns and audits |
| `common/` | Shared provenance helper for evidence recorded before 0.4 |

Each script adds every topic folder to its import path, so shared references (for example
`dungeon_reference`) import by module name from any folder. Frozen artifacts are never
rewritten: evidence recorded before the 0.4 layout keeps its original paths and digests,
and `common/provenance.py` maps them to the current files (`common/moves-0.4.json`). For
runners/sources the 0.4 layout edited, it verifies digests against the last pre-0.4 commit
in git history; that proves what was measured, not that edited code measures the same.

## Query and packing models

`model_eval.py` asks real models the same questions about one game state with the data
as plain JSON and as gamectx packets, then reports accuracy and billed input tokens.
Standard library only; any OpenAI-compatible chat API.

```bash
cargo build --release
python3 -I eval/query/model_eval.py --dry-run                      # packets, questions, cost; no calls
python3 -I eval/query/model_eval.py --key-file PATH --cap 0.50     # stops before spending more than $0.50
python3 -I eval/query/model_eval.py --base-url URL --models a,b --formats json,gamectx
python3 -I eval/query/model_eval.py --kinds inv_filter,npc_filter --formats gamectx-grouped,gamectx-grouped+reason
```

Formats: `json` (compact JSON), `gamectx` (tables), `gamectx-grouped` (tables grouped by
item kind and NPC faction), `gamectx-signatures` (grouped tables, actions as signatures)
and `grouped-sorted` (grouped, rows sorted inside groups). Add
`+reason` to any format to ask for a brief list of candidate rows before the answer.
`--kinds` limits the run to some question types; `--dry-run` lists them all.

The key is read from a file so it never appears in shell history or arguments. Results
go to `eval/results/` (ignored by git). Temperature is 0, but providers do not guarantee
identical answers between runs, so treat differences of one or two answers as noise.

`memory_eval.py` compares sending a 60-event history with sending the 8 events gamectx
recalls for each question (`--dry-run` checks recall without any API calls).

`calc_eval.py` asks arithmetic questions over the same data, answered directly or as an
expression that `gamectx calc` evaluates.

## Query tool

`query_eval.py` prototypes model-written selection, sorting and aggregation in Python.
It compares direct answers, SQL, a JSON query alternative, and the `query` tool packed
with the other actions as a signature. All modes send the full data: this measures
accuracy and request tokens, not savings from omitting tables or total multi-turn cost.

```bash
cargo build --release --locked
python3 -I eval/query/query_eval.py --tokens-only --formats direct,tool --cap 0.003
python3 -I eval/query/query_eval.py --formats direct,sql,json --cap 0.08
python3 -I eval/query/query_eval.py --formats tool --cap 0.03
# No API requests (the key is not read):
python3 -I eval/query/query_eval.py --dry-run --cap 0
python3 -I eval/query/query_eval.py --replay eval/results/query-RUN/results.json --cap 0
python3 -I eval/query/query_eval.py --differential 3000 --cap 0
python3 -I eval/query/test_query_eval.py
```

Measured on 2026-11-05, 29 questions per model, temperature 0, Llama 3.2 3B and Mistral
Small 3.2 24B. The [saved summary](query/query-results.json) records counts, tokens and costs.

| Mode | Llama correct | Mistral correct | Total | Avg input, Llama / Mistral |
|---|---|---|---|---|
| Direct | 17/29 | 16/29 | 33/58 | 1,482 / 1,502 |
| SQL request | 25/29 | 28/29 | 53/58 | 1,569 / 1,586 |
| JSON query | 28/29 | 24/29 | 52/58 | 1,601 / 1,616 |
| Packed query tool (separate run) | 27/29 | 25/29 | 52/58 | 1,622 / 1,638 |

Direct / SQL / JSON: numeric filters 0/6 / 6/6 / 6/6, whole-table questions
2/6 / 6/6 / 6/6, arithmetic 1/16 / 12/16 / 12/16. The packed tool scored 6/6, 5/6
and 11/16 respectively. SQL is the shipped interface: JSON scored one fewer overall,
used more input, and generated invented `group_by` conditions. The small difference
in total accuracy is not evidence that SQL is intrinsically more accurate.

The three-mode run cost $0.0352; the packed-tool run cost $0.0123. A later token-only
check with each mode's instructions included cost $0.000756: direct vs packed tool
1,471 vs 1,611 input tokens for Llama, 1,490 vs 1,626 for Mistral. All four usage
reports omitted cached-token accounting; this does not establish cache support or discounts.
Token-only runs save full usage separately in `tokens.json`, with caps enforced.
Cost accounting uses undiscounted input rates as a conservative estimate.

Rust reproduces all 116 recorded SQL/tool queries' acceptance and result bytes against
the final prototype (including rejections), plus 3,000 generated/mutated queries
(1,527 accepted). These free checks establish implementation parity, not new model
accuracy. That historical experiment uses one synthetic state and one response per
question; it establishes no retry improvement or schema-only accuracy.

## Query workflow and holdout states

`query_workflow_eval.py` is a new experiment; it leaves the historical questions/data
unchanged. `legacy` uses the old weight fields/questions. `full` and `schema` use precise
unit-weight questions and host-derived `stack_weight = count * unit_weight`. The comparison
between legacy and explicit conditions changes both the fields and question wording;
it does not isolate a causal effect of renaming a column.

Each request gets one correction after an execution error. A valid query ends execution,
including when its meaning is wrong. The result then returns to the model for a final
JSON answer. All request, retry and answer-turn usage is counted. A result budget limits
responses to 4096 UTF-8 bytes. Descriptions send types and complete small string domains,
never numeric samples or partial enums. Full/schema prompts include the same actions
and field notes. Tied maxima accept any tied answer.

```bash
cargo build --release --locked
python3 -I eval/query/query_workflow_eval.py --dry-run --seeds 7,101,202,303 --cap 0
python3 -I eval/query/query_workflow_eval.py --tokens-only --cap 0.006
python3 -I eval/query/query_workflow_eval.py --cap 0.12
# Holdout prompts/scoring are fixed before these states' accuracy calls:
python3 -I eval/query/query_workflow_eval.py --tokens-only --seeds 101,202,303 --modes full,schema --cap 0.008
python3 -I eval/query/query_workflow_eval.py --seeds 101,202,303 --modes full,schema --cap 0.24
# Free checks (no key read):
python3 -I eval/query/test_query_workflow.py
python3 -I eval/query/query_workflow_eval.py --check --seeds 7,101,202,303 --cap 0
python3 -I eval/query/query_workflow_eval.py --replay eval/results/workflow-RUN/results.json --cap 0
```

Measured on 2026-11-05, temperature 0, the same two default small models. Development
seed 7: legacy 53/58 first → 55/58 after correction, explicit full 55/58 → 57/58,
explicit schema 56/58 → 57/58. The final-answer turn preserved these scores.

Holdout seeds 101, 202 and 303 were not used to tune the prompts. Their inventories
contain 24, 48 and 96 rows, and NPC tables 10, 20 and 40 rows. State 303 combines two
independent generated populations with disambiguated IDs. Each has 29 questions per model.

| Holdout mode | First correct | After correction / final answer | Avg total input per question | Avg total output |
|---|---|---|---|---|
| Explicit full tables | 160/174 | 171/174 | 4,898.632 | 58.874 |
| Explicit schema-only | 169/174 | 172/174 | 1,615.874 | 56.523 |

Llama scored 87/87 after correction in both modes. Mistral scored 84/87 full and 85/87
schema-only. Full used 14 corrections, recovering 11 answers; schemas used 4, recovering
3. The schema-only errors were one rarity `unique` → count = 1 interpretation and one
top-N subquery that remained unsupported after correction. The three full-table errors
were persistent top-N subqueries. No correct executed result was corrupted by the final
answer turn. The one-answer difference is not a claim of general accuracy superiority.

Complete-exchange input was 67.0% lower with schemas; completed-exchange costs were
$0.107952 full versus $0.038804 schema-only (64.1% less). These count the actual billed
usage across both or three turns, not just the initial context. An incomplete full-table
request at a spending cap cost another $0.00033028125; it is included in total spend and
excluded from completed-exchange token averages. That run was resumed for the four
unfinished questions with identical tables/prompts and a new cap; completed questions
were not repeated. `--resume PATH` supports this workflow; its cap applies to new spend
and `cumulative_spent_usd` includes the previous run. Use the completed resume file in
reports instead of double-counting its parent.

The development accuracy run cost $0.071431. Token-only checks cost $0.001100 for development
and $0.002232 for holdouts. Holdout accuracy runs including the capped/resumed request cost
$0.147087. Total new spend: $0.221850. [Saved measurements](query/query-workflow-results.json)
include per-state results, remaining failures, usage and conservative undiscounted costs.
`summarize_query_workflow.py` rebuilds the summary from saved runs without API calls.

All 547 measured query-attempt envelopes replay byte-identically against Rust; inferred
schemas match on all 12 state/mode combinations. These are multiple states from one
synthetic family, not evidence for arbitrary games. Keep unrelated distributions and real
integration data as further holdouts.

## Local reaction brains

`brain_eval.py` is entirely offline: no API, credential lookup or third-party Python
packages. It prototypes integer-millionth neural inference, guarded training, parameter
lessons, validation gates and saved learning, and compares Rust byte for byte.

```bash
cargo build --release --locked
python3 -I eval/runtime/brain_eval.py                # regenerates eval/runtime/brain-results.json, free
python3 -I eval/runtime/brain_eval.py --check        # prototype/Rust parity, free
python3 -I eval/runtime/test_brain.py                # shared fixtures and atomicity, free
cargo run --example adaptive-brain          # runnable host example, free
```

Labels are synthetic utility functions representing stealth reactions, ally support,
nonlinear switch matching, and trading. One deterministic 4-hidden-node initializer
(seed 3), 32 training samples (seed 11), 32 separate validation samples (seed 23), 128
epochs and learning rate 0.1 are used for each. Diagnostics on seeds 101/202/303 tested
gradient variants before the final evaluation on fresh seeds 707/808/909 (128 samples
per seed/task). Reaction correctness means the highest-utility label matches the reference
argmax; abstention counts as incorrect. These are not LLM-authored-policy accuracy tests.

| Task | Random initialization correct / 384 | Trained correct / 384 | Validation MSE before → after |
|---|---|---|---|
| Stealth | 140 | 372 | 0.115617 → 0.001225 |
| Ally support | 74 | 359 | 0.166728 → 0.002309 |
| Nonlinear switch | 192 | 306 | 0.090474 → 0.007696 |
| Trading | 139 | 384 | 0.099110 → 0 |
| Total | 545/1,536 | 1,421/1,536 | |

Adaptation changes trading's hold utility from 0.15 to 0.35. The existing trained network
gets fresh training/validation seeds 31/47, then is tested on seeds 1001/1002/1003. It
scores 264/384 before and 384/384 after adaptation; validation MSE falls from 0.013335
to 0.000032. These are fresh labels supplied by the host, not discovered causal rewards.
Trained saved states after the first update are 1,374 / 1,762 / 1,381 / 1,587 bytes.
[Saved measurements](runtime/brain-results.json) include work estimates and Python timings for
one local run; timing is not a platform guarantee. Total API spend is $0.

Measured alternatives: a linear-only network scored 192/384 on the switch holdout versus
306/384 with hidden nodes. On diagnostic samples, strict clamp gradients scored
1,424/1,536 but could not recover a saturated old output. Straight-through gradients at
all nodes recovered it but scored only 1,243/1,536. Applying straight-through gradients
only at outputs scored 1,430/1,536 and recovered saturation; this is the shipped rule.
Hidden nodes still use the clamp derivative and can be inactive; initialization matters.

Free parity checks cover 292 complete workflows, including training, serialization and
2,304 predictions (four tasks, linear ablation and adaptation), plus 250 generated/mutated
definition workflows within that count. The 41 new conformance fixtures include errors,
pins, replay, state restore, undo limits, thresholds, unknown inputs, masks and work caps.
Rust additionally mutates 30,000 model-written graph/lesson/training/state texts.

Limits: one initializer and small synthetic task families. Validation-loss improvement
is a supplied-label check, not proof of better action accuracy, trustworthy labels or
out-of-distribution behavior. The switch task's 78 misses remain visible. No billed-token
or LLM accuracy improvement is claimed from these local tests.

## Brain training-rate selection

`brain_accuracy_eval.py` tests opt-in validation-based rate selection, entirely offline.
The original `brain_eval.py` experiment and saved measurements stay historical.

```bash
cargo build --release --locked
python3 -I eval/runtime/brain_accuracy_eval.py           # regenerates selection measurements, free
python3 -I eval/runtime/test_brain_accuracy.py
python3 -I eval/runtime/brain_accuracy_eval.py --check
gamectx brain examples/fixtures/brain/selection.json
```

The four utility tasks use the existing initializer (seed 3, four hidden nodes), training
seed 11 with 32 examples, and validation seed 23 with 128 examples. The validation-only
diagnostics compared data amounts, initializers and rates before freezing selection
rates 0.03/0.1/0.3. Each trial trains for 128 epochs from the same original parameters.
Choose the most correct validation reactions, then lower MSE, then earlier rate order.
Require MSE improvement and no decision-count regression relative to the active brain.
Fresh test seeds 2001/2002/2003 each contribute 128 samples per task; they are not used
for selection. Both compared methods use the same training/validation/test samples.

| Task | Single rate 0.1 / 384 | Selected rate / 384 | Rate selected | Single / selection work units |
|---|---|---|---|---|
| Stealth | 376 | 378 | 0.03 | 275,968 / 833,536 |
| Ally support | 361 | 366 | 0.03 | 388,864 / 1,174,528 |
| Nonlinear switch | 315 | 359 | 0.3 | 275,968 / 833,536 |
| Trading | 384 | 384 | 0.1 | 338,688 / 1,022,976 |
| Total | 1,436/1,536 | 1,487/1,536 | | |

The paired result is 93.5% → 96.8%, 51 fewer errors on these samples. Training uses about
3× estimated work, with no change to inference. This does not compare the old 1,421 score
against a new cohort: that older benchmark used different test seeds and 32 validation
examples. All scores are reference-policy agreement, not measured human/gameplay quality.

Diagnostics explain rejected shortcuts. More examples alone at a fixed 4,096 training
sample-visits improved switch validation MSE (0.012898 → 0.009566) while reducing reaction
correctness (110/128 → 103/128). Rate 0.3 helped switch (119/128) but hurt support
(116/128 versus 119/128 at 0.1). Initializer seeds also had no universal winner; switch
validation ranged 74/128 (seed 1) to 114/128 (seed 7), versus 110/128 (seed 3). Selection
retains an existing brain's authored/learned starting state instead of silently reseeding it.

[Saved results](runtime/brain-accuracy-results.json) include all 32 validation-only diagnostic
conditions and per-rate reports. API spend: $0. Tests cover 17 new conformance fixtures,
45 byte-identical Python/Rust workflows including 1,536 fresh predictions, and 10,000
additional model-text mutations. All old format outputs remain unchanged.

Limits: same synthetic task families and one initializer in final evaluation; all-output
eligibility and zero thresholds. Multiple trials can overfit validation data, and the
metric gate gives no guarantee on unseen distributions or state-dependent host action masks.

## Brain preloading and prediction caching

`brain_cache_eval.py` prototypes a bounded per-owner FIFO cache entirely offline. It
normalizes inputs using the existing fixed-point rules and includes eligibility,
thresholds and revision. `brain_cache_bench.rs` measures the implemented release API,
with byte-equality assertions against uncached predictions outside the timed loops.

```bash
python3 -I eval/runtime/brain_cache_eval.py
python3 -I eval/runtime/test_brain_cache.py
cargo build --release --locked
python3 -I eval/runtime/brain_cache_eval.py --check
rustc --edition=2021 -O eval/runtime/brain_cache_bench.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/brain-cache-bench
./target/brain-cache-bench > eval/runtime/brain-cache-results.json
cargo run --example preloaded-brains
```

The [saved local measurements](runtime/brain-cache-results.json) report medians of five trials,
20,000 predictions per trial, capacity 32. Trial order alternates cached/uncached. Inputs
are prepared before timing. Small networks have four inputs, four hidden nodes and two
outputs (30 parameters); large networks have four inputs, 16 hidden nodes and four
outputs (148 parameters). Both use deterministic dense initialization. `repeated` uses
one state, `working_set` cycles 16 states, and `churn` cycles 256 states through 32 slots.
Timings include validation and owned JSON prediction values; they exclude serialization.

| Network | Pattern | Uncached median ns | Cached median ns | Hits / misses |
|---|---|---:|---:|---:|
| Small | repeated | 31,316,135 | 15,938,668 | 19,999 / 1 |
| Small | working_set | 29,160,784 | 16,990,950 | 19,984 / 16 |
| Small | churn | 29,010,868 | 53,640,316 | 0 / 20,000 |
| Large | repeated | 36,523,550 | 19,222,841 | 19,999 / 1 |
| Large | working_set | 34,732,697 | 19,185,174 | 19,984 / 16 |
| Large | churn | 37,100,149 | 67,275,724 | 0 / 20,000 |

Repeated workloads were 1.7–2.0× faster. The zero-hit workloads were 1.8–1.85× slower,
so caching is optional rather than a new default. No extra input rounding is introduced;
cache hits preserve normal inference exactly. These graphs are small and owned results
still allocate on hits; workload, allocator and hardware affect the result.

The same benchmark compares 1,000 owners, again medians of five trials. The template is
compiled before timing, and saved JSON is serialized before restoration timing. Dense
creation starts fresh learning; it is not equivalent to restoring an adapted brain.

| Network | Clone compiled template ns | Restore saved JSON ns | Fresh dense creation ns |
|---|---:|---:|---:|
| Small | 1,412,118 | 24,237,576 | 12,885,626 |
| Large | 8,952,343 | 142,615,037 | 84,578,382 |

Cloning was 15.9–17.2× faster than restoring JSON per owner. The benchmark uses freshly
initialized saved states; long learning histories can change costs. Parse/validate a
saved brain once at startup, then retain or clone it. Each clone learns independently.

Six new shared cases exercise normalized keys, eligibility, thresholds, unknowns,
mutation/undo, capacity zero and errors. The checker covers 64 Python/Rust workflows
(including all existing brain/selection cases) with byte-identical results and saved
learning. Rust tests check bounded FIFO occupancy, owned results, invalidation even for
same-revision replacement and 10,000 mutated workflow texts against uncached execution.
API spend: $0; no credentials or model calls are used.

## Frame work and context reuse

All experiments here are offline and cost $0. `runtime_eval.py` prototypes exact
packet sizing, native prediction semantics, sample slicing, event scheduling,
host-policy selection and acknowledged context transfer. `runtime_bench.rs` measures
release APIs with allocation/reallocation counts from a single-threaded counting
allocator. `budget_before.rs` retains the old assembly algorithm for comparison; it
uses the same current JSON implementation (including allocation-free `json_len`), so
this isolates assembly changes rather than comparing complete historical binaries.

```bash
python3 -I eval/runtime/runtime_eval.py                  # regenerate deterministic results
python3 -I eval/runtime/test_runtime.py
cargo build --release --locked
rustc --edition=2021 -O eval/runtime/runtime_check.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/runtime-check
python3 -I eval/runtime/runtime_eval.py --check
rustc --edition=2021 -O eval/runtime/runtime_bench.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/runtime-bench
./target/runtime-bench > eval/runtime/runtime-bench-results.json
cargo run --example runtime-loop
```

[Timing/allocation results](runtime/runtime-bench-results.json) are medians of five trials in
one local environment. Native and JSON inputs are prebuilt before timing, cycle 256
states and represent the same millionth-grid observations. Small/large graphs have
four inputs, 4/16 hidden nodes and 2/4 outputs, seed 3. Both APIs perform inference;
JSON also maps names, validates numeric text and builds owned JSON results. Native
success needs no heap allocation. Neither timing includes output serialization.

| Network | Calls | JSON median ns | Native median ns | JSON allocations | Native allocations |
|---|---:|---:|---:|---:|---:|
| small | 20,000 | 26,030,769 | 1,968,370 | 380,000 | 0 |
| large | 20,000 | 32,503,158 | 6,171,925 | 400,234 | 0 |

Native is 5.3–13.2× faster in these workloads; API-path comparison, not a claim that
all inference became that much faster. Prepared input/output storage trades caller
binding work for fewer runtime allocations. JSON inference still matches old outputs.

Packet fixtures contain 16 sections × 128-byte values with a 4,096-byte cap (`fits`),
or 32 sections × 256-byte values with a 2,048-byte cap (`pressure`). Section zero is
required; priorities cycle 0..4 and stable flags alternate. Each timed one-shot call
clones its input sections; prepared references are created before timing. Both cases
perform 500 builds per trial. Prepared fixtures reuse every section, so they represent
maximum reuse, not a claim that all dynamic content can be cached. Results/receipts
are asserted equal before timing.

| Pattern | Old median ns | New one-shot ns | Prepared reuse ns | Old allocations | New allocations | Prepared allocations |
|---|---:|---:|---:|---:|---:|---:|
| fits | 5,587,160 | 6,098,296 | 1,316,569 | 47,000 | 30,500 | 26,000 |
| pressure | 146,774,066 | 19,042,995 | 1,834,731 | 710,500 | 76,500 | 27,000 |

Under pressure one-shot assembly is about 7.7× faster; already-fitting one-shot runtime
was about 9% slower in this recorded run (5,587,160 → 6,098,296 ns), with fewer allocations.
Prepared reuse is about 4.2×/80.0× faster than the old fits/pressure algorithm on these all-static fixtures. Initial always-prepare one-shot
assembly added allocations when packets already fit; the shipped path counts first and
moves fitting values directly. Arbitrary fit predicates retain full candidate callbacks.

Sliced selection uses `examples/fixtures/brain/selection.json`: one observed uninterrupted run
1,307,880 ns versus 25 slices capped at 4,096 work units, total 1,185,084 ns, median
slice 46,973 ns, maximum 53,370 ns. This is a latency illustration, not a benchmark of
scheduler guarantees or a claim that slicing reduces total training work. Setup and
finish occur outside those slice timings. The final report and saved state match direct
selection byte-for-byte. Python also checks 1,024 training visits split into 147 slices
per task over four tasks, preserving final weights exactly.

[Deterministic runtime results](runtime/runtime-results.json): 64 actors over 240 frames,
one unit/evaluation, budget eight/frame, initial phases 0..7, minimum interval one,
refresh age 60 and all actors changed every 30 frames. Scheduled evaluations 512 versus
15,360 per-frame evaluations, peak eight/frame. This models sparse change notifications;
continuously changing or urgent actors need more work, and overload can delay deadlines.

Context trace: 120 turns with stable rules/tools and changing tick/mood. Full states
489,490 bytes; explicit-ACK snapshots/deltas 19,089 bytes, exact replay on every turn.
Consecutive full-state prompts share 483,496 prefix bytes in aggregate. These are bytes,
not tokenizer counts, billing discounts, context-window savings or measured prefill time.
Wrong/lost ACKs cause snapshots; the corpus includes loss, replay and reordering tests.

Host-policy selection freezes training seed 11 (32 examples), validation seed 23
(128 examples), initializer 3 and rates 0.03/0.1/0.3 before test seeds 3001/3002/3003
(128 each). Eligibility cycles empty, first-only, last-only and all-output masks;
minimum score cycles 0/0.2/0.5 and minimum margin 0/0.05/0.15. Both methods evaluate the
same held-out choices under those policies. Selection by all-output validation and
selection by host-policy validation chose identical rates here:

| Task | Prior selection / 384 | Host-policy selection / 384 | Chosen rate |
|---|---:|---:|---:|
| Stealth | 361 | 361 | 0.03 |
| Support | 368 | 368 | 0.03 |
| Switch | 354 | 354 | 0.3 |
| Trade | 383 | 383 | 0.1 |
| Total | 1,466 / 1,536 | 1,466 / 1,536 | |

No measured accuracy improvement on this cohort. The new opt-in metric closes a
validation-semantics gap; synthetic policies still do not represent live gameplay.
Default selection keeps its original tied-target behavior; host selection expects
abstention for tied target winners, using the exact runtime threshold rules.

35 new conformance cases include errors. Differential checking covers 148 workflows,
2,304 native predictions (including all 1,536 held-out host-policy predictions), 100
generated escaped/Unicode budget cases and four complete host-selection states/reports.
Mutation tests add 30,000 sync-message and 10,000 host-training/job text mutations.
All previous fixtures remain unchanged.

### Optional local llama.cpp prefix measurement

```bash
python3 -I eval/query/prefix_cache_eval.py --cap 0
python3 -I eval/query/prefix_cache_eval.py --url http://127.0.0.1:8080 --turns 12 --cap 0
```

Without `--url`, this only accounts for byte prefixes. With a running local server it
compares cache disabled/enabled on explicit slot zero, warms each mode with a distinct
reset prompt and generates one token/turn. It records wall time and whatever token/time
fields the server actually reports; absent counts remain null. Modes run sequentially,
so timing comparisons can include warmup/order effects. Do not use a concurrently
shared slot or assume cached and uncached generations are bit-identical.

The harness accepts only an unauthenticated HTTP loopback origin, disables proxies and
redirects, requires `--cap 0` and never reads credentials. No local executable/server
was available for this work, so actual KV-cache/token savings remain unmeasured.
The request options and response interpretation follow the official
[llama.cpp server documentation](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).
This harness is an integration aid; provider prompt-cache discounts remain unverified.

## Bindings, semantics and evidence

`integration_eval.py` prototypes versioned physical-input bindings, diagnostic replay,
scoped memory, native evidence/hypotheses and optional stable reaction selection.
`evidence_eval.py` supplies the evidence reference implementation. All measurements
except the explicitly recorded query-meaning runs below are offline and cost $0.

```bash
python3 -I eval/runtime/integration_eval.py
python3 -I eval/runtime/test_integration.py
cargo build --release --locked
rustc --edition=2021 -O eval/runtime/integration_check.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/integration-check
python3 -I eval/runtime/integration_eval.py --check
python3 -I eval/query/query_meaning_eval.py --check --cap 0
rustc --edition=2021 -O eval/runtime/integration_bench.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/integration-bench
./target/integration-bench > eval/runtime/integration-bench-results.json
cargo run --example integration-tools
cargo run --example evidence-memory
```

### Packet regression and memory measurements

[Saved packet results](runtime/integration-bench-results.json) compare the frozen `dd87e76`
assembler (`budget_pre_integration.rs`, including local copies of its private JSON
helpers) against the repaired byte-budget path. The prior path walked values for
size and then receipt size before/during serialization. The new fitting path collects
receipt lengths while serializing, borrowing stable sections first without moving
input declarations. A first attempt using another sorted index vector added an
allocation and regressed tiny packets; the final path avoids that extra vector.

Release timing uses seven paired trials, alternating before/after order, with allocation
counters disabled. Allocation/reallocation counts and peak transient bytes come from
separate single-build samples using a counting System allocator. Peak bytes exclude
pre-existing input/prepared storage; every counted allocation is freed within the sampled
closure. Prepared reuse runs after the pair in each trial and is a maximum-reuse scenario.
This differs from the historical instrumented runtime benchmark, so compare the matched
before/after rows here rather than absolute timings across experiments.

| Workload | Builds | Before median ns | After median ns | Allocations before→after | Peak transient bytes before→after |
|---|---:|---:|---:|---:|---:|
| Tiny | 1,000 | 224,347 | 199,188 | 9→9 | 148→164 |
| Fits | 1,000 | 9,171,479 | 5,905,360 | 61→61 | 8,108→8,140 |
| Pressure | 1,000 | 34,371,446 | 29,824,656 | 153→172 | 26,825→26,825 |
| Large | 100 | 34,650,555 | 28,934,700 | 411→432 | 231,497→231,497 |
| Mixed | 1,000 | 8,822,089 | 5,708,635 | 72→72 | 6,034→6,066 |

Tiny: one 8-byte string section, cap 128. Fits: 16×128-byte strings, cap 4096. Pressure:
32×256, cap 2048. Large: 96×1024, cap 8192. Mixed: 16 sections, every fourth a small
object, others 128-byte strings, cap 4096. Priorities cycle 0..4, stable flags alternate,
section zero is required. Input clones occur inside both one-shot timing paths; prepared
references are built outside. Text and receipts are asserted equal before timing.

These fixtures improve throughput about 1.13–1.55×, with pressure-path allocation
increases and small fitting peak-memory increases explicitly shown. They do not establish
an improvement for every payload/allocator/engine. Prepared reuse remains faster here;
its exact rows are retained in the JSON artifact rather than treated as dynamic-content
savings. Native context and generic content predicates retain prior behavior.

### Physical bindings, diagnostics and stability

Bindings retain the full effective schema, not only an ID/hash. Tests change physical
units, range, revision and feature order; saved bindings reject mismatches. Input tests
include inverted/clamped/unknown values and physical ranges crossing zero. This validates
the declared contract, not whether an engine measured what it claimed.

[Deterministic integration results](runtime/integration-results.json) use fresh random seeds
5001/5002/5003 for stability: two direct utility outputs, 240 frames/seed. At each frame,
inputs are independent 0.5±0.01 noisy utilities, except every 40th frame supplies a strong
0.8/0.2 preference and marks urgency. Frames 150..157 allow only the second reaction.
Switch margin is 0.04 and minimum hold is four ticks. Compare raw greedy selection and
held selection over identical observed utilities:

| Seed | Raw switches | Stable switches | Missed urgent switches | Ineligible choices | Total utility regret, millionths |
|---|---:|---:|---:|---:|---:|
| 5001 | 120 | 5 | 0 | 0 | 839,763 |
| 5002 | 119 | 5 | 0 | 0 | 682,109 |
| 5003 | 116 | 5 | 0 | 0 | 759,802 |
| Total | 355 | 15 | 0 | 0 | 2,281,674 |

Regret sums best currently eligible utility minus selected utility; mean about 0.003169
per frame. This is a consistency/utility tradeoff, not a new accuracy score. Test actual
reaction delay and gameplay before enabling holding. These replayed observations cannot
show how a different action changes subsequent world states.

Differential checking covers 152 workflows including all 46 new cases, 720 held-out
stability frames, 720 replay observations and 800 generated binding inputs. Replay
reasons include unknown eligible scores, no eligibility, all-zero/tied utilities and
score/margin thresholds. Label agreement and nearest-rank p50/p95 use supplied host data.
The illustrative trace's 100..160 ns values are fixture data, not measured inference times.

### Query-meaning model check

`query_meaning_eval.py` freezes three fresh states (seeds 6101/6102/6103), 24 item-stack
rows each with randomized IDs, quantities, rarity categories and physical mass. Four
initial rows ensure rarity `unique` and quantity one are distinct concepts in every
state. Ten fixed questions/state cover categories, counts, per-unit mass and per-stack
mass. Both approved small models see table descriptions without rows, with or without
host meanings. Each returns one SQL query; no correction/final-answer turn is included.
Scores use executed result values (ID lists unordered; numeric relative tolerance 1e-6).
Ambiguous natural-language meanings are defined by the host metadata; this is a targeted
semantic test, not evidence that arbitrary gameplay questions improve equally.

The token check ran first, capped at $0.01; the accuracy run was capped at $0.06. Only
Llama 3.2 3B and Mistral Small 3.2 24B ran. [Token usage](query/query-meaning-tokens.json):

| Model | Schema input tokens | Meanings input tokens |
|---|---:|---:|
| Llama 3.2 3B | 205 | 365 |
| Mistral Small 3.2 24B | 190 | 352 |

[Accuracy results and every recorded query](query/query-meaning-results.json):

| Model | Schema correct / 30 | Meanings correct / 30 | Average input, schema→meanings | Average output, schema→meanings |
|---|---:|---:|---:|---:|
| Llama 3.2 3B | 12 | 18 | 207.4→367.4 | 22.3→22.2 |
| Mistral Small 3.2 24B | 24 | 27 | 192.6→354.6 | 17.7→18.1 |
| Total | 36/60 | 45/60 | | |

Token check spend $0.0001380125; accuracy spend $0.0051951; total **$0.0053331125**.
Meanings cost more tokens and still miss 15/60 queries. Provider usage supports these
counts, not a cache discount claim. `--check --cap 0` proves three metadata descriptions
and all 120 executed/error query outputs match Rust byte-for-byte, entirely offline.

```bash
# Paid examples; do not rerun automatically during tests.
python3 -I eval/query/query_meaning_eval.py --tokens-only --cap 0.01
python3 -I eval/query/query_meaning_eval.py --cap 0.06
```

### Freshness, evidence and hypotheses

Journal fixtures distinguish active observations, proposals, expiry and permanent
supersession. The capacity-one case retains an observation over a competing proposal.
An expired replacement does not restore the old superseded observation; a recalled
fallback proposal is still explicitly marked proposed. Existing Memory fixtures remain
unchanged.

Evidence reference tests include six valid and nine invalid workflows: native-token
replay, stale snapshots, omitted/duplicate/unknown citations, changed privacy, wrong
source provenance, confidence limits and byte budgets. Valid workflows exercise supporting
and contradicting refs, repeated claims, expiry, contradiction and forgetting dependencies.
Repeat test: requested confidence 0.6→0.9 and TTL 100→1000; actual confidence stays 0.6,
creation stays 2 and expiry stays 30 (source expiry), with exactly two native observations
before and after. Model guesses never enter the native evidence pool. Sealed-instance
checks, restore subject/clock mismatches and saved metadata consistency have native tests.

The raw persisted store may contain private observations; model context uses filtered,
byte-budgeted snapshots. A hypothesis is omitted when a required source is absent from
the retained packet. Only its retained source IDs are citable in the next reply. Filtering
is immediate; physical forgetting removes evidence and dependent hypotheses. The host owns
consent, source truth, logical clocks, schema meaning and durable archive I/O. Citation
validation proves availability/provenance, not entailment, factual truth or calibrated
confidence, and does not confer action permissions.

Mutation tests add 40,000 binding/metadata/journal/replay texts plus 20,000 cited-proposal
and saved-evidence texts. Errors must not partially alter observations, state or traces.
All earlier 261 conformance fixtures stay unchanged. This phase's total API spend is
$0.0053331125; other integrity and performance checks are offline.

## Registered facts

`facts_eval.py` prototypes explicit native/user assertions, pins, optimistic revisions,
dependency invalidation, privacy inheritance/erasure, exact typed claim comparison and
bounded context tables. It makes no API calls and needs no credentials. Decimal-number
lexemes are retained; comparisons avoid float rounding, including large integer IDs.

```bash
python3 -I eval/runtime/test_facts.py
python3 -I eval/runtime/facts_eval.py
cargo build --release --locked
rustc --edition=2021 -O eval/runtime/facts_check.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/facts-check
python3 -I eval/runtime/facts_eval.py --check
cargo run --example registered-facts
```

[Recorded offline result](runtime/facts-results.json): 128 registered assertions and 128 exact
supported checks; a 1,024-byte cap keeps eight rows in 927 bytes and reports 120 dropped.
The remaining active assertions stay available by direct lookup. This deterministic
fixture exercises size and integrity, not token savings or natural-language/gameplay
accuracy. There is no measured model-accuracy improvement from this feature.

Rust/Python parity covers 119 workflows: all 19 new conformance cases (nine positive,
ten errors) and 100 seeded generated structured values (seed 8107). Expected outputs
were reviewed for claim status, revision changes, dependency invalidation, ordering,
erasure and original numeric spellings. Native tests add transitive chains, stale
updates, cyclic replacements, user ownership, expiry, forged counters, hostile values
and `Turn::facts` integration. An additional 20,000 deterministic mutations exercise
model-written typed claims/values and saved registry text, checking bounded parsing,
no read mutation, atomic rejected registrations and successful restore round-trips.
All previous 307 conformance fixtures remain unchanged. This addition costs $0 API;
the preceding model-meaning token/accuracy checks together cost $0.0053331125.

## Optional runtime upgrades

`upgrades_eval.py` is an independent offline Python prototype for indexed recall,
validated procedures, observable outcomes, temporal features, shared perception frames,
asynchronous context tickets and runtime spending reservations. No API calls, keys or
provider libraries are used. Original model tools and all earlier 326 conformance cases
remain unchanged.

```bash
python3 -I eval/runtime/test_upgrades.py
python3 -I eval/runtime/upgrades_eval.py
cargo build --release --locked
rustc --edition=2021 -O eval/runtime/upgrades_check.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/upgrades-check
python3 -I eval/runtime/upgrades_eval.py --check
rustc --edition=2021 -O eval/runtime/upgrades_bench.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/upgrades-bench
./target/upgrades-bench
cargo run --example runtime-upgrades
```

[Deterministic results](runtime/upgrades-results.json):

| Check | Result |
|---|---|
| Selective recall, 100 queries/1,000 records | 100,000 scan record visits versus 100 index postings |
| Optional normalization/host aliases | 2/2 illustrative matches |
| Repeated validated procedure | 100/100 checks return a draft |
| Observed training labels | 100; zero unexposed labels; zero duplicate labels |
| Shared perception, 100 frames ×12 candidates | 2,400 naive versus 1,300 shared module evaluations |
| API spend | $0 |

The recall fixture uses unique marker terms; common terms can still visit every record.
Index construction, storage and rebuild costs are excluded from the visit count. It is
not an embeddings benchmark or general retrieval-accuracy result. Procedures offer local
reuse opportunities, not measured avoided model calls. Host-observed labels do not prove
an action caused a response. Temporal rates require actual host observation coverage.

Differential checks cover 283 workflows: 40 conformance cases (18 positive/rejection-report
workflows, 22 errors), 240 generated recall/outcome/temporal/ledger workflows and three
bundle cohorts with 100 frames/12 candidates each. Fresh seeds 9101/9102/9103 include
unknown perception inputs; all 3,600 candidate predictions also match duplicate native
module evaluation exactly. Unit tests cover cross-instance tickets, changed contexts,
privacy/capacity/expiry, current argument validation, failed atomic updates, module teaching
isolation, source replay and conservative ledger restore. Expected outputs were reviewed
for exact source/version/phase semantics. In particular issued and current request
contexts are independent copies; changed actor/world/schema/permissions must return false.

[Paired release benchmark](runtime/upgrades-bench-results.json), seven alternating trials, median
nanoseconds for 1,000 frames ×12 candidates; both paths use existing native inference,
prebuilt modules and the same inputs. Bundle preparation is included; construction is
excluded. Checksums agree before and during timing; native unit tests compare full results.

| Hidden nodes/module | Duplicate perception ns | Shared perception ns |
|---|---:|---:|
| 0 | 729,906 | 631,514 |
| 16 | 3,341,042 | 2,014,761 |

These timings cover two synthetic sizes on one machine, not engine frame times. A
prepared identity/scope allocates once per cycle; candidate `run_into` uses stack storage.
Measurements do not imply equal gains for every module mix or candidate count.

Mutation checks add 14,000 seven-kind workflow texts plus 40,000 authored configuration,
saved module/ledger and request-context texts. Parsers have byte bounds, native data
has count/depth limits, failed primitive updates stay atomic, and successful loads
round-trip. Ledger recovery deliberately settles pending reservations; second same-time
restore is stable. No live paid requests occur in CI. The [format contract](../docs/FORMAT.md#optional-runtime-upgrades)
separates library checks from host sensing, validation, archive and billing duties.

## Fresh alpha.3 model check

The [2026-11-06 report](../docs/evidence/MODEL_TEST_2026_11_06.md) records a new hosted-model run
using Llama 3.2 3B and Mistral Small 3.2 24B on six fresh states across cargo, teams and
routes. Eight questions/state/model/format give 288 scored exchanges. Full JSON and
lossless packed tables receive identical meanings and rows; schema + query receives
native schema/meanings, executes Rust sessions and asks for a final answer.

| Method | Correct / 96 | Average complete-exchange input |
|---|---:|---:|
| JSON | 27/96 | 1,786.792 |
| Packed | 27/96 | 1,252.125 |
| Schema + query | 91/96 | 916.490 |

Packing reduces input 29.9% at the same aggregate score; querying reduces input 48.7%
including corrections and final answers. Individual JSON/packed outcomes differ. Queries
score 83/96 first, 91/96 after correction and 91/96 final; five misses remain. The report
includes per-model/family counts and all original replies, questions, oracle queries,
field meanings, prompts, usage and price snapshots. This is a synthetic tabular benchmark,
not a test of live gameplay, brain policies or memory reasoning.

Token check: $0.0050195625 (cap $0.01). Cumulative accuracy spend: $0.05027535625 (cap
$0.12), including $0.0000777 for one incomplete exchange after an adapter error. Total:
$0.05529491875. The original interrupted run is retained; 15 completed cases were reused
without requesting their answers again. Only the interrupted exchange was restarted.
Neither prompts nor scoring were tuned after looking at results.

Offline commands (CI uses these; no credential or model API access):

```bash
python3 -I eval/query/test_release_model.py
python3 -I eval/query/release_model_eval.py --check --cap 0 --output eval/query/release-model-tokens.json
python3 -I eval/query/release_model_eval.py --check --cap 0 --output eval/query/release-model-results.json
python3 -I eval/query/summarize_release_model.py --check
```

The checker verifies six native prompt sets/48 independent oracle results and 105
recorded Rust/Python feedback envelopes. Regression tests cover nonterminal session EOF,
strict JSON/scoring, conservative reservations, unknown usage, metering overruns and
resume without repeated completed requests. The report generator verifies identities,
frozen inputs, source/script hashes, ledger totals and score recomputation. No Rust
library format or existing conformance case changes.

Paid reruns require new paths and explicit caps; preserve the token-first sequence:

```bash
mkdir -p eval/results
python3 -I eval/query/release_model_eval.py --tokens-only --cap 0.01 --output eval/results/release-tokens-new.json
python3 -I eval/query/release_model_eval.py --cap 0.12 --output eval/results/release-accuracy-new.json
```

The runner makes one transport attempt per call and checkpoints the full reservation
before dispatch. Unknown/failed calls retain their reservation and stop. `--resume FILE`
reuses completed cases with identical tables/prompts/pricing; its cap includes prior
cumulative spend, not a fresh allowance. Nonterminal Rust session EOF is accepted only
when the CLI returned exactly one expected retriable feedback envelope; the next step
reconstructs both calls and checks complete native/Python parity. Final-answer generation
is never skipped when a query succeeds.

## Query execution and identifier matching

Offline prototypes: `query_optimization_check.py` (streaming arithmetic),
`query_upgrade_eval.py` (plans/contracts/bounds), and `relevance_eval.py` (identifier
components). The alpha.4 executor is frozen in `query_before.rs`; only its schema module
path is adjusted for standalone compilation. Query parser text, default tool/session
formats, arithmetic order, selected values and validation precedence are preserved.

Seven paired alternating release trials, parsed-query reuse, allocator counters disabled
for timing. Each allocation/peak sample runs separately and drops all transient values
inside its tracking window. Tiny rows have an 8-byte payload; other rows have a 1,024-byte
payload, plus ID/mass/units. Measurements are local synthetic execution, not model latency,
frame deadlines or guaranteed speedups on different row layouts.

| Fixture | Rows | Calls/trial | Before ns | After ns | Allocations/query before→after | Peak transient bytes before→after |
|---|---:|---:|---:|---:|---:|---:|
| tiny | 1 | 10000 | 7,210,976 | 1,579,555 | 14→3 | 360→64 |
| wide-total | 1024 | 1000 | 1,277,473,628 | 53,773,386 | 9,229→11 | 1,355,791→8,211 |
| filtered-list | 1024 | 1000 | 1,336,165,565 | 41,170,030 | 9,236→12 | 1,380,335→429 |
| top-n-total | 1024 | 1000 | 1,714,696,196 | 133,320,872 | 9,223→12 | 1,429,487→65,536 |
| text-sort | 1024 | 1000 | 1,359,804,249 | 87,516,281 | 10,251→15 | 1,443,737→65,536 |

Observed parsed-execution ratios are 4.6–32.5×. Parsing cost is excluded for both variants;
`Query::parse` already supported reuse before this change. The executor now borrows row
references/sort keys and streams aggregate inputs. Only selected cells are cloned.

Checks: 4,000 Python streaming aggregate comparisons; 2,024 native alpha.4/current
result and exact error-string comparisons, including signed zero, nulls, late invalid
rows, mixed sort types, overflow/cancellation and LIMIT 0; 3,000 generated/mutated
Rust/Python queries (1,527 accepted); 516 plan/shape/bounds comparisons; all 105 old
recorded feedback envelopes unchanged. Thirty-five new conformance cases bring the total
to 401; all 366 previous outputs remain byte-identical. Added 20,000 deterministic
mutations for contracts/plans/identifier text. API spend for these checks: $0.

Ten identifier lexical regressions improved 2/10 → 10/10, with four additional catalog
parity cases. Namespace/dotted/path boundaries `: . / -` and underscore components are
recognized without substring matching. Original IDs, Unicode strings and large-number
spellings are retained. This is lexical correctness, not semantic or model accuracy;
memory recall/index tokenization keeps its existing contract.

Two measured rejections are retained in `query-upgrade-offline.json`: exact oracle shape
would reject 27/96 old queries, including 22 correct equivalents, so structural contracts
require independent host requirements. Flattening IN/top-N subqueries is unsafe with
ties: `[10,9,9,9]` gives top-three sum 28 but IN-membership sum 37. Neither is an automatic
query repair.

```bash
cargo build --release --locked
rustc --edition=2021 -O eval/query/query_plan_check.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/query-plan-check
rustc --edition=2021 -O eval/query/query_compare.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/query-compare
python3 -I eval/query/test_query_upgrade.py
python3 -I eval/query/query_optimization_check.py
python3 -I eval/query/relevance_eval.py
python3 -I eval/query/query_eval.py --differential 3000 --cap 0
python3 -I eval/query/query_upgrade_eval.py --cap 0
rustc --edition=2021 -O eval/query/query_bench.rs -L dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o target/query-bench
target/query-bench
```

The measurement artifact is `query-performance.json`. Re-running timings prints new
local measurements; it need not rewrite this recorded run.

## Optional query guidance: fresh paired test

The tested supplemental paragraph is available through `query::Guidance::ExplicitSelection`
and CLI `query --guidance explicit-selection`; `Standard` adds no text. Native profile
text matches the frozen Python paragraph exactly. Four profile conformance cases include
unknown-name/non-string errors. No automatic choice based on model size is added.

Fresh six-state cohort (same three schema families/question templates, new row seeds),
48 questions/model/profile: 3B standard 46/48→explicit 48/48; 24B 47/48→46/48.
Aggregate first queries 86/96→84/96, after correction/final 93/96→94/96. Two paired
improvements and one regression. Complete-exchange input 907.865→1,107.802 (+22.0%).
Keep standard as the default; the optional profile is a workload/model tradeoff, not a
universal accuracy upgrade. All 209 fresh native/Python feedback envelopes match.

Token-only check cost $0.0012702 (cap $0.01); accuracy $0.0281033125 (cap $0.08);
total $0.0293735125. Full replies/usage/pricing are in `query-guidance-tokens.json` and
`query-guidance-results.json`; audited derived statistics in `query-guidance-summary.json`.
See the [full report](../docs/evidence/QUERY_OPTIMIZATION_2026_11_06.md) for methodology and limitations.

```bash
python3 -I eval/query/query_upgrade_eval.py --check eval/query/query-guidance-results.json --cap 0
python3 -I eval/query/summarize_query_upgrade.py --check
```

Future paid runs require new output paths and explicit caps, token-only first:

```bash
python3 -I eval/query/query_upgrade_eval.py --models --tokens-only --cap 0.01 --output eval/results/query-guidance-tokens-new.json
python3 -I eval/query/query_upgrade_eval.py --models --cap 0.08 --output eval/results/query-guidance-results-new.json
```

## Integration kit (offline, unreleased)

`kit_eval.py` independently prototypes registration, selection, packing, argument validation,
brain bindings and recorded-run scoring. `python3 -I eval/runtime/kit_eval.py --check` compares
144 workflows (113 accepted, 31 rejected) byte for byte with `target/debug/gamectx`.
It includes 101 generated stock snapshots and three separately seeded scripted evaluation
states. Expected results are not model-generated. `kit-results.json` records this run.

`kit_bench.rs` measures registration reuse against reparsing the same definition on every
turn. Compile against the release rlib like the existing benchmarks. Seven alternating
paired trials, 1,000 turns each: 146,130,805 ns versus 78,931,400 ns median, identical output;
raw samples in `kit-performance.json`. Both arms validate each snapshot. This is synthetic
registration reuse, not model latency or a comparison against hand-optimized host code.

Build the separate native crate, then run `kit_bindings_check.py` (265 Python/native checks)
and `node eval/runtime/kit_bindings_check.mjs` (8 Node checks). The Python script can produce
C# fixtures with `--csharp-fixtures DIR`; compile `bindings/csharp/{GamectxClient,Check}.cs`
and pass the project and generated requests/expected paths. Linux run: 59 fixture replies
and two smoke checks. The Rust suite adds 47 conformance cases and 20,000 text mutations.

No API/network requests are made by these scripts. The public `gamectx evaluate` command
scores host-collected runs over independent expected snapshots; guidance is metadata,
missing usage stays null, and incomplete/duplicate cohorts fail. No new model-accuracy
claim. New API spend $0. See [KIT](../docs/guides/KIT.md) for formats and limits.

## Local 1B–3B deployment contracts (v0.2.0a)

`local_reference.py` and `local_cases.py` prototype full-request reservations, protected
scene/dialogue hydration, exact legal-pair decisions, explicit deployment profiles and
complete-turn reporting. New API docs: [LOCAL](../docs/guides/LOCAL.md). Run
`python3 -I eval/local/local_cases.py --check` after building `target/debug/gamectx`; it compares
148 workflows (132 accepted, 16 rejected) byte for byte. Forty-eight new conformance cases
include errors; all 448 previous cases are unchanged. Mutation tests add 20,000 inputs.
The fixture byte counter is explicitly estimated, not a model tokenizer.
`local_bindings_check.py` adds 296 Python/native checks and generates 48 C# fixture
replies for the existing C# driver; CI exercises both transports.

`ollama_eval.py` is opt-in, requires `--cap 0`, uses standard-library HTTP with loopback-only
origins and no redirects/credentials/pulls/unloads. Run `--tokens-only` first. Accuracy
conditions isolate raw-vs-scene selection and JSON-vs-schema, with optional concise/example,
native tools and narration. Every model invocation, preflight, correction and result is
recorded; unmatched token preflights count inside complete-turn time and usage. Hydration,
local query work and validation are inside the timer. Natural cache reuse is not disabled.
Do not benchmark while compiling or running other heavy workloads.

`test_ollama.py` exercises a fake streaming endpoint, malformed replies, local-only URL
validation, independent affordability oracles and missing-metric semantics without a model.
`check_local_model.py --cap 0 ARTIFACT...` recomputes decision/report outputs and checks
complete cohorts, actual prompt counts, retained required facts and raw-offer oracles.
Use fresh output files; partial checkpoints remain explicit. Linux memory values are sampled
RSS/available RAM/swap deltas, not true allocator peaks. Missing cache metrics stay null.

The [local report](../docs/evidence/LOCAL_MODEL_2026_11_06.md) links frozen prompts, responses, settings,
per-call usage/timing and measured hardware. No hosted API spend is incurred. Existing paid
cohorts and their scripts remain frozen. Prose quality is independently reviewed, not inferred
from schema compliance or a correct action ID.

## Further optimization contracts (unreleased)

`optimization_reference.py` prototypes seven additive host workflows in the suggested
order: intent boundaries, typed query predicates, dependency-complete evidence, external
full-render counting, local-match routing, readable choice codecs and split configuration
comparison. `optimization_cases.py --check` compares compact Python/Rust output and
rejections; `--check-bindings` checks persistent Python and native C ABI paths. Independent
expected conformance outputs are generated by the Python prototype, not blessed from Rust.

`optimization_measure.py --cap 0 --check` audits frozen byte/guard/routing/scoring fixtures.
`--check-tokens optimization-tokens.json` audits six saved one-token local prompt probes.
The original packet is 419→49 bytes, typed example arguments 56→15 bytes (schema 206),
and Qwen 2/4/8-choice prompt counts are 158→118, 224→174, 344→274 with schema grounding.
These are different measurements; byte figures are not tokenizer savings.

`optimization_model_eval.py` uses loopback Ollama only and requires `--cap 0`. It requires
a completed matching token-only artifact before the accuracy run, pulls no models, and
records complete raw streams/usage. Twelve synthetic scenes × two formats scored 8/12
pairs versus 11/12 named; input 3,865→3,285, output 178→111, no retries/fallbacks. Both
conditions preserve the same offered order. The first pilot used different offered
orders and is excluded; `optimization_model_pilot.py` freezes its checker and artifacts.
Offline CI replays results; it makes no model calls. All new API spend is $0.

See [the workflow guide](../docs/guides/OPTIMIZATION.md) for assumptions, limits and commands.
The new accuracy probe is not a full-turn latency/RAM study or proof of live game gains.

## Playable dungeon testbed

`dungeon_reference.py` independently implements the original example's deterministic
rules. `dungeon_cases.py --cap 0 --check --audit eval/lantern-vault/dungeon-results.json` verifies 103
Python/Rust replays, six scripted wins and 240 packet observations. Build with
`cargo build --example dungeon --locked` first. Twenty-three separate example conformance fixtures add no changes
to the 565 core cases. Tests add 10,000 malformed replay mutations.

`dungeon_agent.py` optionally runs an already installed local GGUF model through loopback
Ollama, requiring `--cap 0` and successful matching initial token probes before gameplay.
It caps calls/turns, validates proposals locally and records actual trajectories. A native
one-token preflight precedes generation/correction; timing includes these phases and host
work. Missing model RAM is not invented. `check_dungeon_agent.py --cap 0 FILE...` audits
saved model text/actions against both independent Python rules and the native host.

The first named-mode Qwen/seed-7 smoke exposes repeated legal `look_entry` actions with no
quest progress. Preserve it as a failure, not a successful legal-output score. Both local
runs spend $0 API; CI only replays saved data. The [guide](../examples/lantern-vault/README.md)
separates byte/checkpoint figures from token/model claims and includes playable commands.


## Human-play workflows and entity references (unreleased)

[WORKFLOWS](../docs/guides/WORKFLOWS.md) records exact counts, remaining prose errors and
limits. All new runs use the same installed local Qwen 3.09B Q4_K_M, context 2048,
four CPU threads. Every token and accuracy run spends $0 API; no larger model is loaded.

Offline reproduction:

```sh
cargo build --locked
cargo build --example dungeon --locked
python3 -I eval/local/workflow_cases.py --cap 0 --check
python3 -I eval/lantern-vault/dungeon_social_cases.py --cap 0 --check
python3 -I eval/lantern-vault/test_dungeon_ai.py
python3 -I eval/lantern-vault/check_dungeon_slots.py --cap 0 --check eval/lantern-vault/dungeon-slots-tokens.json eval/lantern-vault/dungeon-slots-results.json eval/lantern-vault/dungeon-entities-tokens.json eval/lantern-vault/dungeon-entities-results.json
python3 -I eval/lantern-vault/check_dungeon_social.py --cap 0 eval/lantern-vault/dungeon-social-tokens.json eval/lantern-vault/dungeon-social-results.json eval/lantern-vault/dungeon-social-guided-tokens.json eval/lantern-vault/dungeon-social-guided-results.json eval/lantern-vault/dungeon-social-focused-tokens.json eval/lantern-vault/dungeon-social-focused-results.json
```

`workflow_cases.py` independently checks 24 size/task presets plus two render successes
and four errors, byte-identical to Rust. `dungeon_social_cases.py` checks 43 conversation
workflows and 13 example fixture sets. `check_dungeon_slots.py --check` compares 360
entity workflows across six seeds against independent Python rules. Its positional
artifacts replay full offers, raw calls, budget reservations and scores. Host deferrals
are separate from model correctness. Core fixture cases remain independent of this game.

For new local runs use **fresh output paths**, token-only first, then a matching token
artifact. The scripts bound generation reservations (64 for speech cohorts, 32 for slot
cohorts), pin deployment identity, use loopback only, and never download models or read
API credentials. `--cap 0` is mandatory. Token probes and corrections count toward the
local call cap, including failed dispatches.

```sh
python3 -I eval/lantern-vault/dungeon_social_eval.py --cap 0 --tokens-only --guidance focused --model arzaan789/qwen2.5-3b-uncensored:latest --output eval/results/social-tokens.json
python3 -I eval/lantern-vault/dungeon_social_eval.py --cap 0 --guidance focused --model arzaan789/qwen2.5-3b-uncensored:latest --tokens eval/results/social-tokens.json --output eval/results/social-results.json
python3 -I eval/lantern-vault/dungeon_slots_eval.py --cap 0 --tokens-only --entity-context --model arzaan789/qwen2.5-3b-uncensored:latest --output eval/results/entity-tokens.json
python3 -I eval/lantern-vault/dungeon_slots_eval.py --cap 0 --entity-context --model arzaan789/qwen2.5-3b-uncensored:latest --tokens eval/results/entity-tokens.json --output eval/results/entity-results.json
```

Standard/grounded/focused character cohorts each make 26 calls with no correction.
The focused variant uses fewer input/output tokens, but still fails 2/4 interpretations
without local guards. Slot-only pilot 7/8, phrase-aware regression pilot 8/8; the latter
uses four local deferrals plus four correct model decisions. These reused regression
questions do not establish held-out semantic accuracy. Raw prose contradictions remain.
New scripts record individual call times, not complete-turn latency or runner RSS;
controlled end-to-end latency/retry/memory work remains necessary before promotion.
CI replays all frozen data without generation. Old failed dungeon planning data remain.


## Shared conversation contracts and local trading (unreleased)

`conversation_reference.py` and `references_reference.py` independently specify the new
library contracts; `conversation_cases.py --cap 0 --check` compares 78 workflows and
writes 38 additive conformance fixtures with `--write-conformance`. Existing 595 cases
and earlier model artifacts stay unchanged. The shared Rust APIs replace example storage,
reply validation and entity matching without requiring an LLM rerun for byte parity.

`dungeon_trade_reference.py` models the separate game purchase policy.
`dungeon_trade_cases.py --cap 0` compares 66 workflows / 144 trade requests over six seeds
against the real host, including insufficient funds, one-use acceptance, decline, stale
confirmation, topic changes, owned items and ambiguous targets. `test_dungeon_ai.py`
now has 11 real-host/fake-generation tests, including the actual conversation loop.
All these tests make zero model calls and spend $0 API. Core mutations add 30,000
attempts. See [CONVERSATION](../docs/guides/CONVERSATION.md) for bounds, limitations and APIs.

```sh
cargo build --locked
cargo build --example dungeon --locked
python3 -I eval/lantern-vault/conversation_cases.py --cap 0 --check
python3 -I eval/lantern-vault/dungeon_trade_cases.py --cap 0
python3 -I eval/lantern-vault/test_dungeon_ai.py
```

After building the native bridge, `conversation_cases.py --cap 0 --check-bindings`
compares the same cases through persistent and native transports; CI runs both.


## User-reported dialogue corpus

`dungeon-feedback.json` preserves 11 reported sequences / 26 turns, with manual claim
annotations and missing deployment/usage fields explicitly null. These are observations,
not fresh model calls or an aggregate accuracy score. After building the dungeon,
`python3 -I eval/lantern-vault/check_dungeon_feedback.py --cap 0` independently audits seed-7 host
reproductions: 17 trade requests, 16 matching local receipts, nine supplied speech packets
and two purchases in separate successful-purchase sequences. It checks unchanged world
state for speech and original-player-only quote retention against the Python reference.
The current validator accepts some factually wrong speech; acceptance is not correctness.
No model is called; new API spend $0. CI runs the offline audit. [Report](../docs/evidence/DUNGEON_FEEDBACK.md).


## Authored lore and source-bound local answers

[LORE_EVAL](../docs/evidence/LORE_EVAL.md) records the full pilot history, token/accuracy usage,
latency definitions and remaining failures. All runs use the same installed Qwen 3.09B
Q4_K_M, four CPU threads, context 2048; every run spends $0 API. Frozen v1–v4 reference
policies retain failed routing. Current policy is `dungeon_knowledge_reference.py`.

```sh
cargo build --locked
cargo build --example dungeon --locked
python3 -I eval/knowledge/lore_cases.py --cap 0 --check
python3 -I eval/lantern-vault/dungeon_knowledge_cases.py --cap 0
python3 -I eval/lantern-vault/test_dungeon_knowledge.py
python3 -I eval/knowledge/check_lore_model.py --cap 0 eval/knowledge/lore-tokens.json eval/knowledge/lore-results.json eval/knowledge/lore-v2-tokens.json eval/knowledge/lore-v2-results.json eval/knowledge/lore-native-tokens.json eval/knowledge/lore-native-results.json eval/knowledge/lore-fresh-tokens.json eval/knowledge/lore-fresh-results.json eval/knowledge/lore-retest-tokens.json eval/knowledge/lore-retest-results.json eval/knowledge/lore-extra-tokens.json eval/knowledge/lore-extra-results.json eval/knowledge/lore-repair-tokens.json eval/knowledge/lore-repair-results.json
```

There are 108 independent core workflows, 28 new conformance cases, 60 byte-identical
native example request workflows, eleven front-end checks and 30000 added mutations. After
building the native bridge, `lore_cases.py --cap 0 --check-bindings` checks both transports.

For new generation, `lore_native_eval.py --cap 0 --cohort extra --tokens-only --output
<fresh-file>` precedes a matching `--tokens <file>` accuracy run. Native cohorts are
regression/fresh/extra/repair; all older question sets are now known regression data.
Freshness labels describe when questions were first introduced, not an untouched holdout
on reruns. Calls are capped at 96 including preflights/corrections/failures. Complete-turn
timing starts before native hydration and ends after source validation/conversation finish;
scenario setup and extra audit state fetch are excluded. Per-turn missing memory stays null.
`lore_memory.py --cap 0 --results <artifact> --output <fresh-file>` takes a separate warm
post-run memory sample, not a peak or per-turn measurement. No script pulls weights,
changes services, contacts a hosted endpoint or reads credentials. CI only audits saved data.


## Optional informational tools and natural dialogue

Offline, no server or key:

```bash
cargo build --locked --bin gamectx --example dungeon --example info_tools
python3 -I eval/knowledge/info_inventory_cases.py --cap 0
python3 -I eval/lantern-vault/dungeon_natural_cases.py --cap 0
python3 -I eval/lantern-vault/check_natural_model.py --cap 0 eval/lantern-vault/natural-tokens.json eval/lantern-vault/natural-results.json eval/lantern-vault/natural-v2-tokens.json eval/lantern-vault/natural-v2-results.json
python3 -I eval/knowledge/check_lore_model.py --cap 0 eval/knowledge/lore-stock-tokens.json eval/knowledge/lore-stock-results.json
cargo run --locked --example info_tools
```

39 format workflows, 96 stock regressions and 72 native natural requests match independent
Python prototypes. All 661 earlier conformance fixtures stay unchanged. [Information
contracts](../docs/guides/INFO.md) quote schema/result byte sizes from the executable example.

`dungeon_natural_eval.py` requires an already-installed local model, `--cap 0`, a fresh
output path, and a token-only artifact before generation. `--version 1` reconstructs the
frozen broad prototype; default version 2 uses narrower facts. Both retained trials have
20/20 structurally valid replies but factual failures. Do not equate shape acceptance with
accuracy, and do not infer latency/RAM improvements from these overlapping diagnostics.
[Raw-result interpretation](../docs/evidence/LORE_EVAL.md).


## Optional conversation tools

```bash
cargo build --locked --bin gamectx --example conversation_tools
python3 -I eval/knowledge/interaction_cases.py --cap 0
python3 -I eval/knowledge/interaction_measure.py --cap 0 --check eval/knowledge/interaction-results.json
cargo run --locked --example conversation_tools
```

87 Python/Rust workflows, 47 new conformance fixtures (700 prior unchanged), 30000 mutation
attempts. The 120-event byte fixture selects two records from 80 authorized events:
8589→231 bytes. Eight schema bytes total 3127; enable only useful tools for each turn.
All checks are offline, $0 API, and do not establish live-model accuracy or latency.
[Privacy, multiplayer and consent contracts](../docs/guides/INTERACTIONS.md).


## Information-tool transport regressions

After building the CLI and native library:

```bash
python3 -I eval/knowledge/information_bindings_check.py --cap 0 --fixtures target/information-csharp
node eval/knowledge/information_bindings_check.mjs --cap 0 target/information-csharp
mcs -out:target/information-csharp/Check.exe bindings/csharp/GamectxClient.cs bindings/csharp/Check.cs
LD_LIBRARY_PATH=bindings/native/target/debug mono target/information-csharp/Check.exe examples/kit/project.json target/information-csharp/requests.txt target/information-csharp/expected.txt
```

126 independent information/inventory/conversation workflows cross both Python transports
(252 checks), Node (126) and C# native (126): **504 transport checks**, including Unicode,
reported evidence, rejected source/owner injection, private events, promise progress,
recipient confirmation and stale/replayed offers. CI now runs these checks. API spend $0;
these checks exercise native behavior, not live-model tool-selection accuracy.

## Optional routine/time contract (unreleased v0.3.1a)

`python3 -I eval/character/routines_cases.py --cap 0` checks an independent Python prototype
against Rust on 69 deterministic workflows (63 accepted, six rejected). Twenty-one
additive core fixtures retain all 797 prior outputs. `--prototype-only` works without
Rust; `--write-conformance` writes only the new feature cases from the oracle.
`routine_bindings_check.py --cap 0` generates/checks 43 persistent fixtures on Python
CLI/native; `routine_bindings_check.mjs` and the existing C# fixture runner reuse them.
New Rust mutation tests make 20,000 attempts, without model calls or network access.
These are host-correctness checks, not live model accuracy/token/latency/RAM measurements.
Historical evaluation artifacts are untouched. New model/API spend $0.

Follow-up: `python3 -I eval/character/routine_improvements_cases.py --cap 0` checks 54 independent
selection/timer/dependency workflows and audits `routine-performance.json` (seven paired
host-only timings, no provider calls). Nine new conformance cases preserve all earlier
818 outputs. The noisy observation fixture measures selected JSON bytes 2,102→371 with
a required older source retained; this is not a full-request token count or accuracy score.
`routine_detail_bindings_check.py --cap 0` and the Node/C# runners check the expanded
52-fixture session suite. Ten thousand additional mutations test selectors.

To rerun host timings (results vary by machine/load): build release, compile
`eval/character/routine_bench.rs` against `target/release/libgamectx.rlib`, and run `target/routine-bench`.
Both timing arms assert byte-equivalent final state; setup is excluded. The recorded
paired process RSS includes both arms, and excludes models. No live model was loaded.


## Adversarial and local source-classification probes

Offline checks: `adversarial_probe.py --cap 0 --require-fixed` (57 targeted checks and
900 independent engine actions), `adversarial_cases.py --cap 0` (96 time conflicts,
162 command comparisons, 64 typed assessments), `time_assessment_cases.py --cap 0`
(ten additive fixtures), `test_adversarial.py` (10,000 text mutations) and
`test_count_cache.py` (fake transport, identity/count/budget/eviction boundaries).
`adversarial_bindings_check.py --cap 0` plus Node/C# shares 61 persistent fixtures.

Live `character_limit_eval.py --cap 0 --tokens-only --output NEW` must precede its
matching generation run (`--tokens TOKENS --output NEW`). It accepts only the installed
pinned local Qwen tag, CPU4/GPU0/context2048, and never pulls/loads another model. The
recorded 16-state/two-format cohort uses 32 token checks plus 64 turn calls. JSON
validity is 32/32, source classification 25/32; not an NPC speech/gameplay score.

`count_cache_eval.py --cap 0 --tokens-only --output NEW` precedes the paired adapter
experiment. `--production --tokens TOKENS --output NEW` uses the actual opt-in UI cache;
the earlier independent prototype remains reproducible without `--production`. The old
adapter-only pilot and complete diagnostic production timing have different scopes.
`check_character_limits.py --cap 0 --check eval/local/character-limit-summary.json` reconstructs
all requests/scores/count receipts and validates all five retained live artifacts offline.
Every recorded run spends $0 API; combined 159 local calls. No new model speech correctness
or general context-size/parameter-capability claim. [Full evidence](../docs/evidence/ADVERSARIAL_EVAL.md).

## Dynamic character implementation stages

All independent contracts run offline, without credentials or models:

```sh
cargo build --bin gamectx --example dungeon --locked
python3 -I eval/character/life_cases.py --cap 0
python3 -I eval/character/continuity_cases.py --cap 0
python3 -I eval/character/episodic_cases.py --cap 0
python3 -I eval/character/initiative_cases.py --cap 0
python3 -I eval/character/continuity_context_cases.py --cap 0
python3 -I eval/lantern-vault/natural_scaled_check.py --cap 0
cargo build --manifest-path bindings/native/Cargo.toml --locked
python3 -I eval/character/life_bindings_check.py --cap 0
node eval/character/life_bindings_check.mjs
```

The first five scripts accept `--prototype-only` and total 221 independent workflows;
`--write-conformance` writes only their new families, never historical expectations.
The 1,000-event archive workload is offline. C# uses the generated `target/life-csharp`
requests/expected fixture files through the existing driver. New schemas/defaults and
benchmarks are described in [LIFE](../docs/characters/LIFE.md) and [LIFE_EVAL](../docs/evidence/LIFE_EVAL.md).

For the structural validator, run `validation_reference.py` into
`target/validation-oracle.json`, build the release library, then compile/run
`validation_check.rs` and `validation_alloc.rs` as the CI workflow shows. `life_bench.rs`
can be compiled against an isolated `fedab27` release library and the current release
library; `life-performance.json` records seven alternating pairs and exact final-state
identity. No benchmark invokes a model. These are CPU/allocator measurements only.

`life_model_eval.py` accepts only the installed local model, loopback transport and
`--cap 0`; it never pulls/unloads models or changes service settings. A token-only run
must precede generation with matching identity/fixtures; generation has eight turns,
one preflight plus generation each. Use fresh output paths: retained outputs cannot be
overwritten. Audit frozen `life-model-{tokens,results}.json` with `--check`; failed prose
and assistant review remain in `life-model-review.json`, with human review unknown.
A complete new pair makes 24 local calls, $0 API; no hosted budget is inferred.

## Life speech v2 follow-up

`life_speech_reference.py` independently specifies turn policy, subject-scoped facts,
direct current-request rendering and punctuation diagnostics. After building the CLI,
`python3 -I eval/character/life_speech_cases.py --cap 0` checks 32 compiler workflows and 16 sentence/
bound cases; `--prototype-only` runs just the oracle, `--write-conformance` adds only new
families, and `--check-bindings` additionally checks 64 persistent CLI/native replies.
No old conformance output or v1 speech artifact is regenerated.

`life_model_v2_eval.py --cap 0 --tokens-only --output FRESH_PATH` must precede generation
with `--tokens TOKEN_PATH --output FRESH_PATH`. It uses only the installed local Qwen model,
exact v1 candidate-style baselines, four regression cases and four new paired holdouts.
A complete pair makes 48 calls including 16 token-only checks and 16 in-turn preflights.
The output schema stays at 240 characters and output reservation at 96. All raw replies are
retained; helper diagnostics do not trim, silently repair, retry or replace them. Existing
v1 files are SHA-256-bound and remain frozen. Audit frozen v2 tokens/results with `--check`.
`life_model_v2_review.py --cap 0 --check` reproduces separately labeled assistant notes and
mechanical metrics; it is not independent human scoring. [Results and remaining failures](../docs/evidence/LIFE_SPEECH_V2_EVAL.md).

## Archetype and multi-party host checks

`archetype_reference.py` and `party_compile_reference.py` independently prototype four
composable loadouts, current-attitude gates, relative hearing history, floor reservations
and spotlight routing. Run `python3 -I eval/character/archetype_cases.py --cap 0 --prototype-only
--tokens-only` before implementation/evaluation: no provider/tokenizer is assumed, and
token counts are explicitly unknown. After building the CLI, `--check
eval/character/archetype-party-results.json` checks 68 workflows, including 12 compiled-turn cases.
`--write-conformance` writes only the additive archetype-party family.

`--check-bindings` checks 136 persistent CLI/native bridge replies and creates fixtures in
target/archetype-party-csharp. Run `node eval/character/archetype_bindings_check.mjs`; the existing C#
Check driver accepts that directory's requests.txt/expected.txt. All make zero model calls.
`--report FRESH_PATH` writes deterministic byte measurements, refusing existing paths.
The retained report measures 3,210/2,708 trusted/untrusted provider-neutral request bytes,
zero gated payloads in the untrusted request and zero ambient reserved model calls.
These are structural host checks, not a conversational or model-tier benchmark. Existing
v1/v2 life-model artifacts stay frozen. [Method, limits and runnable example](../docs/characters/ARCHETYPES.md).

## Living world and cast packs

`living_reference.py` and `living_compile_reference.py` independently specify the one-file
cast, bounded attributed reports, chronological group influence, relevant badges/atmosphere
and speech lifecycle. Run `python3 -I eval/character/living_cases.py --cap 0 --prototype-only
--tokens-only` first. No provider is contacted; tokenizer counts remain unknown. After
building the CLI, `--check eval/character/living-world-results.json` checks 54 workflows and frozen
life-model artifact hashes. `--write-conformance` writes only the new living-world family.
`--check-bindings` checks 108 Python CLI/native replies and creates Node/C# fixtures under
target/living-world-csharp; run `node eval/character/living_bindings_check.mjs` and the existing C#
Check driver with that directory's requests.txt/expected.txt.

`living_measure.rs` measures 1,000 unpaced release frames, four existing routine runtimes,
initiative, 64 roots, 128 delivered reports, bounded pair scans and one fallback lifecycle
per frame. It uses 200 fixed JSON strings as simulated completions, not actual model outputs.
Build the release library, then `rustc --edition=2021 -O eval/character/living_measure.rs -L
dependency=target/release/deps --extern gamectx=target/release/libgamectx.rlib -o
target/living-measure`. Run to a fresh JSON path, then `living_measure_check.py --cap 0
--capture RAW_PATH --output FRESH_PATH` records source hashes/hardware/method limits. Audit
with `--check eval/character/living-world-timing.json`; CI does not rerun or enforce timing thresholds.
These are library host-work measurements, excluding engine rendering/physics, real worker
latency and external callbacks. [Results and responsibilities](../docs/characters/LIVING_WORLD.md).

## Host limits and scale timing (offline)

`limits_cases.py --cap 0` checks 25 cases (`conformance/limits/`, 6 malformed) where the pass 2
modules run with host-set limits; `--random 2000 --seed 7` mixes all eight module generators with
random limits against `gamectx local -` (0 mismatches: 1,436 accepted, 564 refused, 1,077 with
limits). `limits_reference.py` is the independent limits envelope.

`limits_character_cases.py --cap 0` checks 25 cases (`conformance/limits_character/`, 6 malformed) for
the pass 1 modules; `--random 2000 --seed 7` runs growth workflows with random limits against
`gamectx local -` (0 mismatches: 1,336 accepted, 664 refused, 1,083 with limits).

`scale_measure.rs` fills twenty-six modules with 1,000 and then 10,000 entries and times 1,000 single-op
queries each, counting live heap with a counting allocator. Build the release library, then
`rustc --edition=2021 -O eval/character/scale_measure.rs -L dependency=target/release/deps --extern
gamectx=target/release/libgamectx.rlib -o target/scale-measure`. Run it to a fresh JSON path, then
`scale_measure_check.py --cap 0 --capture RAW_PATH --output FRESH_PATH` records source hashes, hardware
and method limits. Audit with `--check eval/character/scale-timing.json`; CI compiles and runs the
harness but enforces no timing thresholds. [Results](../docs/characters/LIFE.md#host-set-limits).

`knowledge/limits_knowledge_cases.py --cap 0` checks 25 cases (`conformance/limits_knowledge/`, 6 malformed)
for the knowledge kinds (places, timeline, things, species, recipes, communities, actions, lore, inventory,
evidence, info); `--random 2000 --seed 7` runs eleven generators with random limits near each fixture's size
against `gamectx local -` (0 mismatches: 1,472 accepted, 528 refused, 1,196 with limits). The module
references take their limits from `character/limits_reference.py`. `knowledge/scale_measure.rs` loads
1,000 and 10,000 entries per kind (evidence 256/1,024, facts 1,000/4,096) and times 1,000 queries; build and
retain it like the character harness, with `knowledge/scale_measure_check.py` and
`--check eval/knowledge/scale-timing.json`. [Results](../docs/characters/LIFE.md#knowledge-modules).

## Cooperation and conflict (offline)

`character/cooperation_reference.py` is the independent prototype of pass 2 B2/B3 (joint tasks, help,
hand-offs, promises, claims, queues, contests, grudges). `character/cooperation_cases.py --cap 0` checks 25 cases
(`conformance/cooperation/`, 6 malformed). `--random 2000 --seed 7` builds workflows op by op, keeping ops the
reference accepts (a quarter end on their first refused op), with random limits, against `gamectx local -`
(0 mismatches: 1,446 accepted, 554 refused, 77 helps accepted, 2,149 contests resolved, 1,946 grudge
proposals). [Results](../docs/characters/LIFE.md#cooperation-and-conflict-pass-2-b2b3).

## Bounded rationality and fallible memory (offline)

`character/cognition_reference.py` is the independent prototype of pass 2 A7/A8 (temperament settings, seeded
decisions, fading memories, marked distortions). `character/cognition_cases.py --cap 0` checks 25 cases
(`conformance/cognition/`, 6 malformed). `--random 2000 --seed 7` builds workflows op by op like the cooperation
generator, with a hazy-memory preamble in 30% of them, against `gamectx local -` (0 mismatches: 1,560 accepted,
440 refused, 999 impulses, 214 slips, 223 distorted recalls of which 96 merged a person, 1,178 memories
forgotten). [Results](../docs/characters/LIFE.md#bounded-rationality-and-fallible-memory-pass-2-a7a8).

## Off-screen conversations (offline)

`character/chatter_reference.py` is the independent prototype of pass 2 B6 (topics known and wanted, seeded
meetings, tells, disputes, secrets, gossip, alliances, quarrels). `character/chatter_cases.py --cap 0` checks 25
cases (`conformance/chatter/`, 6 malformed). `--random 2000 --seed 7` builds workflows op by op like the
cooperation generator against `gamectx local -` (0 mismatches: 1,438 accepted, 562 refused, 4,566 tells, 1,278
disputes, 1,025 quarrels, 31 alliances). [Results](../docs/characters/LIFE.md#off-screen-conversations-pass-2-b6).

## Life cycle and threat response (offline)

`character/survival_reference.py` is the independent prototype of pass 2 C1/C2 (lifespans, stages, pairs,
children, heirs, grief, danger appraisal and responses, safety). `character/survival_cases.py --cap 0` checks 25
cases (`conformance/survival/`, 6 malformed). `--random 2000 --seed 7` builds workflows op by op like the
cooperation generator, with a family preamble in 30% of them, against `gamectx local -` (0 mismatches: 1,473
accepted, 527 refused, 4,612 births, 2,791 deaths, 320 belongings inherited, 1,249 threat responses).
[Results](../docs/characters/LIFE.md#life-cycle-and-threat-response-pass-2-c1c2).

## Level of detail for minds (offline)

`character/detail_reference.py` is the independent prototype of pass 2 D2 (full, coarse and crowd levels by
distance, exact catch-up, work by level, crowd summaries). `character/detail_cases.py --cap 0` checks 25 cases
(`conformance/detail/`, 6 malformed). `--random 2000 --seed 7` builds workflows op by op like the cooperation
generator against `gamectx local -` (0 mismatches: 1,481 accepted, 519 refused, 1,754 urgent events, 2,792 level
moves, 449 promotions out of a crowd). [Results](../docs/characters/LIFE.md#level-of-detail-for-minds-pass-2-d2).

## Save growth: retention and migration (offline)

`character/retention_reference.py` is the independent prototype of pass 2 D3 (retention rules and reasoned
pruning, numbered field migrations with bounded work). `character/retention_cases.py --cap 0` checks 25 cases
(`conformance/retention/`, 7 malformed). `--random 2000 --seed 7` builds workflows op by op like the cooperation
generator against `gamectx local -` (0 mismatches: 1,477 accepted, 523 refused, 994 entries pruned, 3,746
documents migrated with 608 edits). `--soak` runs a seeded 30-day town of 50 beings through the reference and
checks the result against `character/retention-soak.json` (945,450 kept bytes with rules vs 3,367,277 without).
[Results](../docs/characters/LIFE.md#save-growth-retention-and-migration-pass-2-d3).

## Visual compiler and approved asset metadata (offline)

`visual_reference.py` independently prototypes bounded art contracts, live expression/pose
selection, perceived-only fact cues, SHA-256 content keys and approval/fallback/repair.
`python3 -I eval/media/visual_cases.py --cap 0 --prototype-only --tokens-only` checks predeclared
expectations before Rust comparison. Build the CLI then run `--cap 0 --check
 eval/media/visual-results.json` for 64 byte-identical workflows (13 malformed). The artifact
records 37 queued metadata jobs, 5 exact reuses, 2 approved fallbacks and frozen hashes of
all six life-model artifacts. Jobs are not dispatched: 0 model calls / $0 spend; image
quality, provider tokens, latency and frame rate unmeasured. `--write-conformance` only
writes the additive visual family. Each accepted expected output was reviewed for selected
rules/cues, withheld facts, state transitions, fallback refs and stale-work rejection.

`--check-bindings` exercises Python CLI/native persistent operations and emits Node/C#
fixtures to target/visual-csharp. Run `node eval/media/visual_bindings_check.mjs` and the existing
C# fixture driver. Tests include original SHA-256 known answers, typed live appraisal and
routine/started-initiative adapters, repair-cycle nonce recovery and 9,000 deterministic
input mutations. [Full contract and limits](../docs/guides/VISUAL.md).

## Loopback RPG / provider / voice studio (offline)

`studio_reference.py` independently prototypes cumulative multi-modality reservation and
Unicode voice receipt policies. `studio_cases.py --cap 0 --prototype-only --tokens-only`
predeclares 32 workflows before comparison. Build the CLI/example, then run
`python3 -I eval/studio/studio_check.py --cap 0 --check eval/studio/studio-web-results.json` to reproduce
32 byte-identical Python/Rust policies, 85 real loopback HTTP endpoint checks and five
fake-browser device/event scenarios. `--write-conformance` on the policy script writes
only the additive studio family; selected routes, reservation errors, heard prefixes and
terminal failures were reviewed individually. All six life-model artifact hashes stay frozen.

`cargo test --example dungeon --locked` includes eleven original studio tests with bounded
fake transports, a blocked worker, local mock HTTP, actual tier IDs, partial hearing,
final-only STT dispatch, post-restart pinning and registry exhaustion. Robustness adds 6,000
policy mutations. All fixtures use cap-zero transports; positive accounting amounts are
synthetic ledger exercises. No installed provider, actual microphone or browser service is
contacted. The retained artifact reports zero real provider calls / $0 spend, with physical
voice offsets/quality, image quality, installed-runner latency and FPS explicitly unmeasured.
See [the complete integration guide](../docs/studio/STUDIO.md).

Overnight investigation: `overnight_deep.py --cap 0 --offline --output NEW.json`
exercises 1,000 archival events and 600 loopback chat turns. Token-first installed-Qwen
cohorts use `--tokens-only`, then `--tokens FILE`, with separate never-overwritten
artifacts; `--holdout` selects separately authored names/preferences. Prototype-only
`overnight_memory_cases.py --cap 0 --prototype-only` and `overnight_input_cases.py`
record repair expectations before Rust changes. See `docs/OVERNIGHT_DEEP_EVAL.md` for
ranked failures, raw observations and diagnostic false negatives.

`overnight_memory_probe.rs` is a standalone release trace linked to this crate, recording
4,000 original events, 40 checkpoint restores and GlobalAlloc payload counts. The frozen
legacy/selected/optimized artifacts and review distinguish deterministic retrieval from
model recall. `overnight_causality_cases.py --cap 0` checks a fresh 132-operation social
arc/rumor cycle against the independent oracle. `overnight_bindings_check.py --cap 0`
checks stateful Python CLI/native selection and prepares Node/C# fixtures in target/.

`overnight_grounded_eval.py --cap 0` predeclares separate paired free/source-bound packets,
requires a token-first artifact, reserves before subsequent requests, and can continue an
interrupted artifact into a new file using `--resume`. The original incomplete run is
retained. `overnight_audit.py --cap 0 --check eval/overnight/overnight-grounded-review.json` audits
48 recorded generations and revalidates all authorized replies in Rust without models.
`overnight_ui_tokens.py --cap 0 --output NEW.json` measures the actual focused GUI body;
`overnight-tier-requests.json` retains actual mock provider requests for all six tiers.

`overnight_input_cases.py --cap 0` compares the command grammar with the independent
prototype. `overnight_input_holdout.py --cap 0` exercises 41 real loopback requests;
`overnight_scope_holdout.py --cap 0 --expect-rejection --output NEW.json` tests delayed
player-switch commands. `test_studio_context.mjs --cap 0` checks six offline browser
request/poll scenarios; it supplements the frozen five audio lifecycle scenarios.
The `--semantic` causality fixture corrects the new arithmetic fixture's reused narration
without rewriting that earlier fixture or any project baseline expectations.

`overnight_campaign_live.py --cap 0 --build --holdout --output NEW.json` creates four
1,000-event sessions and forty fresh restores. First build the public host adapter with
`rustc --edition=2021 eval/overnight/overnight_campaign_compile.rs -L dependency=target/release/deps
--extern gamectx=target/release/libgamectx.rlib -o target/overnight-campaign-compile`
after `cargo build --release --locked` and building the native library (or use `--cli`).
The default renderer applies PartyScene gates; `--legacy-renderer` is an explicit failed
harness reproduction, not a recommended host. `--optional-preferences` exercises archived
lexical preference selection without mandatory preference anchors. Build and tokenize
before live generation; `--required` selects question-specific required answers.
`overnight_campaign_audit.py --cap 0 --check eval/overnight/overnight-campaign-review.json` audits
the frozen failed and corrected live cohorts without calling a model.
`overnight_required_speech_cases.py --cap 0` checks four independent policy workflows.
The final optional preference trace reproduces all twelve previously live-tested packets;
it is not counted as another live model cohort. The complete report records 230 local
calls and the possible single interrupted unrecorded call, with $0 API spend.


Studio usability follow-up: `studio_setup_cases.py --cap 0` declares eight independent
setup/reset expectations; `studio_setup_holdout.py --cap 0 --output NEW.json` exercises
the actual host with an original loopback Ollama mock. It tests installed-name discovery,
apply-after-travel, Talk routing, fallback/recovery, reset while a response is held, stale
results and saved branches. `node eval/studio/test_studio_setup.mjs --cap 0` checks pure UI helpers.
No real provider runs in these automated checks. The original Studio artifact stays fixed.

For a manual real-browser check, start an isolated Firefox with `--headless --no-remote --profile PROFILE --remote-debugging-port=9223`, start an isolated offline preview at
port 8788, then run `python3 -I eval/studio/studio_setup_browser.py --cap 0 --port 8788 --webdriver 9223 --output NEW.json`. This uses the original stdlib loopback BiDi driver,
not a product dependency; it modifies that preview's game and branch A. Physical audio,
STT accuracy, image quality and general player usability are outside its claims.

`studio-setup-live.json` is a separate one-call manual installed-Qwen wiring check,
not part of the offline CI suites or an accuracy benchmark. It records host-validated
speech and explicitly identifies the absence of retained raw generation wire output.

Model manager: `model_manager_cases.py --cap 0` checks 32 independent Python/Rust policy
workflows; its `--prototype-only` and `--write-conformance` modes retain the format oracle.
`model_manager_host.py --cap 0` runs isolated Ollama/SD mocks for routing, local lifecycle,
cache/approval and online-cap fallback. `model_manager_owned.py --cap 0` tests real
owned fixture processes, replacement and parent-exit cleanup. Neither contacts installed
models or online providers. Use `--output NEW.json` to retain a fresh run.
`model_manager_bindings.py --cap 0` checks Python CLI/native and prepares shared Node/C#
fixtures; `node eval/studio/model_manager_bindings.mjs --cap 0` checks the Node bridge.

Manual browser coverage: `python3 -I eval/studio/model_manager_browser.py --cap 0` requires an
isolated offline host on 8788 and Firefox BiDi on 9223. Manual installed-provider lifecycle
smoke: `python3 -I eval/studio/model_manager_live.py --cap 0 --model INSTALLED_ID`; it uses existing
loopback servers, loads/unloads only normal profiles, and leaves normal SD restored.
Neither is an automated CI prerequisite. Retained browser/live JSON records the limits.

Native audio: `python3 -I eval/studio/native_audio_cases.py --cap 0` (13 pure cases),
`python3 -I eval/studio/native_voice_host.py --cap 0` (original CLI fixtures), and
`python3 -I eval/studio/native_audio_bindings.py --cap 0` / `node eval/studio/native_audio_bindings.mjs --cap 0`.
LAN: `python3 -I eval/studio/lan_session_host.py --cap 0` and `python3 -I eval/studio/lan_model_host.py --cap 0`.
Manual Firefox: `python3 -I eval/studio/voice_lan_browser.py --cap 0` against an isolated host on
8788 with `--lan 127.0.0.2` and Firefox BiDi 9223. Manual installed voice smoke:
`python3 -I eval/studio/native_voice_live.py --cap 0 --installation INSTALL_DIR`.
The latter two are not automated CI prerequisites; retained JSON states their limits.

Buffered speech checks: `python3 -I eval/studio/speech_buffer_cases.py --cap 0`,
`node eval/studio/test_speech_buffer.mjs --cap 0`, plus the native-host recovery cohort.
Bindings use `eval/studio/speech_buffer_bindings.py` / `.mjs`. Manual installed timing:
`python3 -I eval/studio/speech_buffer_live.py --cap 0 --installation INSTALL_DIR --baseline BASELINE_BINARY`;
`--holdout-only` tests a fresh multi-part line. Manual Firefox capture uses
`eval/studio/speech_mic_browser.py --cap 0 --reference ORIGINAL_WAV`; `--delay-interim` forces
an interim/final ordering race. It requires an isolated native-STT host on 8788 and
Firefox BiDi 9223. These manual installed/browser checks are not CI prerequisites.
Retained speech-buffer/mic JSON records the measurement limits; old artifacts stay frozen.

Voice history: `python3 -I eval/studio/voice_archive_cases.py --cap 0` (12 pure cases),
`python3 -I eval/studio/voice_history_host.py --cap 0` (original delayed Ollama/CLI mocks),
and the corresponding `voice_archive_bindings.py` / `.mjs` fixtures.
Manual browser: `python3 -I eval/studio/voice_history_browser.py --cap 0` requires Firefox
BiDi 9223 and creates its own isolated mock host. Media playback is stubbed; no model,
physical audio or paid API is invoked. Retained voice-history JSON records its limits.

Character viewpoints: `python3 -I eval/studio/viewpoint_cases.py --cap 0` and `eval/studio/personality_host.py --cap 0` run offline. `eval/studio/personality_browser.py --cap 0` uses an explicitly started Firefox BiDi instance on 9223. Frozen `personality-model-*` and fresh `personality-model-v2-*` retain token-first packets, raw local 3B replies and lexical reviews; [interpretation](../docs/characters/CHARACTER_VIEWPOINTS.md). Bindings use `viewpoint_bindings.py` / `.mjs`.

Reactions: `reaction_cases.py --cap 0` checks 22 independent Python/Rust fixtures; `reaction_host.py --cap 0` checks transfers/debts/replay with no AI. `reaction_browser.py --cap 0` uses Firefox BiDi 9223. Frozen `reaction-host.json` and `reaction-browser.json` record counts/limits. `reaction_bindings.py` / `.mjs` cover all bindings.

Social routing: `social_routing_cases.py --cap 0` runs 22 independent Rust/Python cases; `conversation_host.py --cap 0` exercises production host sequences. `social_conversation_browser.py --cap 0` needs Firefox BiDi 9223. `social_routing_bindings.py` / `.mjs` cover bindings. Frozen `conversation-model-*` and `conversation-model-v2-*`, `conversation-model-v3-*` and `conversation-model-final-*` retain raw token-first/local paired sequences; see [limits and the experimental metadata/duplicate/weather failures and the final zero-call routine path](../docs/studio/STUDIO_CONVERSATION.md).

Contextual greetings: `greeting_memory_cases.py --cap 0` checks 26 independent fixtures; `greeting_memory_host.py --cap 0` and `greeting_actions_host.py --cap 0` exercise real offline host state. `greeting_memory_browser.py --cap 0` needs an explicitly started Firefox BiDi 9223. `greeting_memory_bindings.py` / `.mjs` cover bindings; `studio_stage_reference.py --cap 0` records the original finite action grammar. Frozen new host/browser JSON contains counts/limits. No live model run is needed for this zero-generation layer.

`python3 eval/studio/deliberation_cases.py --cap 0` checks 28 independent reasoning/hypothesis fixtures; `node eval/studio/autospeech_check.mjs --cap 0` checks 11 playback eligibility conditions. No live reasoning quality improvement is claimed.

Conversation extensions: `message_chunks_cases.py --cap 0` (22),
`roleplay_actions_cases.py --cap 0` (14), `conversation_extensions_host.py --cap 0`
(10 workflows/43 HTTP) and `conversation_extensions_browser.py --cap 0` (6 real
Firefox workflows with mock audio/model). `conversation_extensions_model.py` requires
`--tokens-only` first, then a local installed `--model` with cap zero. Retained paired
artifacts and manual review report complete packets but a name-summary omission and
higher wall time; they do not establish quality or performance improvements.

Expression/AI wording: `python3 eval/studio/expression_cases.py --cap 0` (60),
`python3 eval/studio/wording_cases.py --cap 0` (14), `roleplayer_campaign.py --cap 0` and
`ai_wording_host.py --cap 0` use actual host state with offline mocks. Both host scripts
accept `--tokens-only --model INSTALLED_NAME` before a local four-thread live run;
`--output` refuses overwrites. `expression_browser.py --cap 0` uses isolated Firefox
BiDi 9223 and mocked model/audio. Bindings use `expression_bindings.py`/`.mjs`.
Frozen `expression-model-*`, `roleplayer-*` and `ai-wording-*` retain packets, raw local
responses and failures; [interpretation and limits](../docs/studio/STUDIO_EXPRESSION.md).
These scripts never require a paid API, model download or physical voice in CI.

`expression_replay.py --cap 0` replays frozen campaign output through the old chunk
check and new novelty filter with zero model calls. `roleplayer-review.json` separates
mechanical adoption, manual quality observations and remaining limitations, including
an aborted four-call attempt that wrote no raw artifact. AI wording's `--holdout` uses
a different name and request phrasing; capture `--tokens-only` first.

Scene/story repairs: `story_scene_cases.py --cap 0` checks sixteen additive Python/Rust
fixtures. `scene_story_host.py --cap 0` tests twelve selected plots; `--restore-check`
also checks checkpointed rotation. Local `--model` requires a prior `--tokens-only` run.
`scene_voice_check.mjs --cap 0` tests wait state and exclusive ownership;
`scene_voice_browser.py --cap 0` requires isolated Firefox BiDi 9223, and simulates
Piper audio-start refusal plus delivery across two tabs. Existing artifacts remain frozen.
