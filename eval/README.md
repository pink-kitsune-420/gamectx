# Model evaluation

`model_eval.py` asks real models the same questions about one game state with the data
as plain JSON and as gamectx packets, then reports accuracy and billed input tokens.
Standard library only; any OpenAI-compatible chat API.

```bash
cargo build --release
python3 -I eval/model_eval.py --dry-run                      # packets, questions, cost; no calls
python3 -I eval/model_eval.py --key-file PATH --cap 0.50     # stops before spending more than $0.50
python3 -I eval/model_eval.py --base-url URL --models a,b --formats json,gamectx
python3 -I eval/model_eval.py --kinds inv_filter,npc_filter --formats gamectx-grouped,gamectx-grouped+reason
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
python3 -I eval/query_eval.py --tokens-only --formats direct,tool --cap 0.003
python3 -I eval/query_eval.py --formats direct,sql,json --cap 0.08
python3 -I eval/query_eval.py --formats tool --cap 0.03
# No API requests (the key is not read):
python3 -I eval/query_eval.py --dry-run --cap 0
python3 -I eval/query_eval.py --replay eval/results/query-RUN/results.json --cap 0
python3 -I eval/query_eval.py --differential 3000 --cap 0
python3 -I eval/test_query_eval.py
```

Measured on 2026-11-05, 29 questions per model, temperature 0, Llama 3.2 3B and Mistral
Small 3.2 24B. The [saved summary](query-results.json) records counts, tokens and costs.

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
python3 -I eval/query_workflow_eval.py --dry-run --seeds 7,101,202,303 --cap 0
python3 -I eval/query_workflow_eval.py --tokens-only --cap 0.006
python3 -I eval/query_workflow_eval.py --cap 0.12
# Holdout prompts/scoring are fixed before these states' accuracy calls:
python3 -I eval/query_workflow_eval.py --tokens-only --seeds 101,202,303 --modes full,schema --cap 0.008
python3 -I eval/query_workflow_eval.py --seeds 101,202,303 --modes full,schema --cap 0.24
# Free checks (no key read):
python3 -I eval/test_query_workflow.py
python3 -I eval/query_workflow_eval.py --check --seeds 7,101,202,303 --cap 0
python3 -I eval/query_workflow_eval.py --replay eval/results/workflow-RUN/results.json --cap 0
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
$0.147087. Total new spend: $0.221850. [Saved measurements](query-workflow-results.json)
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
python3 -I eval/brain_eval.py                # regenerates eval/brain-results.json, free
python3 -I eval/brain_eval.py --check        # prototype/Rust parity, free
python3 -I eval/test_brain.py                # shared fixtures and atomicity, free
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
[Saved measurements](brain-results.json) include work estimates and Python timings for
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
