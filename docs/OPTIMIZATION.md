# Optional optimization workflows (v0.3.0a)

These additions preserve v0.2.0a's existing formats and defaults. They use only Rust's
standard library and support Rust 1.82. The executable [example](../examples/optimized-turn.rs)
shows host-declared intent and typed query arguments. These are host interfaces; do not
expose the `local` workflow interface wholesale as a model tool.

## 1. Preserve intent during correction

`Choices::with_intent` accepts `{request:{id,text},facts,allowed}`. The host supplies a
nonempty subset of the original legal pairs. Its schema and validator restrict proposals
to that subset. A legal `open` proposal therefore fails when the host permits only
`inspect`/`wait`. A fallback outside the subset becomes abstention. Revision and live
availability checks, one correction, terminal-session checks and atomic host dispatch
requirements remain the same as ordinary sessions. Ordinary sessions are unchanged.

On retry, `correction` contains the original request/facts and a short instruction.
Count that complete correction request before dispatch. Treat request/fact text as data;
never infer an allowlist from a model's self-reported intent. If the host cannot determine
intent, preserve the request but use clarification or the existing model workflow rather
than asserting a guessed restriction. The guard establishes host-policy compliance, not
natural-language understanding or factual truth.

## 2. Register typed query operations

`query::Template::from_value` accepts exactly:

`{name,description,sql,parameters:[{name,column,op,kind}],max_rows,result_bytes}`.

The host SQL uses the existing SELECT grammar. All parameters are required; arguments
cannot choose tables, columns, operators, expressions or limits. `bind` appends predicate
values directly to an independent query AST, retaining host filters. There is no SQL text
substitution. Query filtering/arithmetic retain existing f64 semantics; selected values
retain original JSON number spellings. This does not add joins or subqueries.

Kinds are `string`, `number`, `boolean`; operators are `= != < <= > >= in`. `in` requires
1..64 scalar values of the declared kind. Strings have at most 256 Unicode characters;
numbers must be finite f64 values. There are at most 16 combined constant/bound predicates.
The definition is at most 32,768 bytes, description 2,048, arguments 65,536; maximum input
rows and result bytes are independently bounded at 65,536. Limits are not CPU deadlines.

Register `template.tool()` using `kit::Tool::from_function`, authorize it in the recipe,
and implement its host callback. Use `Project::query_template` in that callback to preserve
recipe visibility/projection/age/row limits, including on empty tables. That method alone
does not authorize a named tool. Alternatively, `Template::run` operates on already
actor-authorized rows. A result cap is checked after computation and never truncates.

## 3. Keep dependent evidence together

`selection::Evidence` accepts at most 128 host-filtered records:
`{id,value,depends:[id],priority,required}`. IDs are unique lowercase identifiers, at most
32 bytes. Dependencies are explicit, with at most 16 per record; missing references,
duplicates and cycles fail. Input and selected packet caps are at most 262,144 bytes.

Required records bring their entire transitive dependency closures. Those must fit.
Optional roots are attempted in descending priority with input-order ties. A root is
included only with its closure; shared dependencies appear once. The result is
`{packet,kept,omitted,bytes}`, in input order. Greedy selection is deterministic, not an
optimal knapsack solver. The byte cap covers packet JSON, not host diagnostics or full
model framing. Feed the resulting packet into the full-request budget.

Dependencies mean "if A is retained, retain B". A dependency may also be independently
selected. For an indivisible group with no independent members, put its fields in one
record value. The selector does not discover relationships, certify provenance, resolve
conflicts, or filter private records. Existing fact/evidence APIs and the host retain
those responsibilities. A memoized traversal bounds work on densely shared DAGs.

## 4. Count full renders from other languages

`request::Composition` implements a counting handshake. Start from
`{id,identity,sections:[{id,value,priority,required}],budget:{context,output,safety}}`.
The full existing deployment identity is mandatory. Definitions are at most 524,288 bytes,
with 128 sections. Every state contains `{status,sequence,candidate,budget,omitted}`;
`candidate` contains `{id,identity,packet}`.

For `status:"count"`, render the candidate using the actual chat template, messages,
tools, response schema and history. Return `{sequence,candidate,count:{input,method}}`
through `observe`. Candidate equality includes JSON object order and numeric spellings; preserve both when echoing it.
The sequence is also checked before state changes.
On overflow the lowest-priority optional section drops (later input wins a drop tie),
and a NEW count is requested. Required overflow ends with `status:"overflow"`; a fit ends
with `status:"ready"`. Terminal sessions reject further observations; malformed observations
leave state unchanged. A new candidate has `budget:null` until counted, preventing use of
an older candidate's receipt. At most 129 observations are useful. Each acknowledgement
is bounded at 786,432 bytes.

The counter belongs to the host. `method:"exact"` remains a trusted assertion, never proof
of correct tokenization. Recheck whenever framing/tools/history change; use a fresh request
ID and session. The library cannot verify hidden template changes, count provenance,
expiry or live dispatch state. Keep all supplied data actor-visible and enforce output
limits at the runner. No generation call is required when the host has a real tokenizer.

The existing JSON scene workflow retains its byte estimator; this additive handshake
supplies a production integration path without changing that old contract. Stateless
JSON callers replay their definition and observations; native Rust callers keep a session.

## 5. Route existing local matches before an LLM call

`decision::route` composes host outputs from rules, validated playbook procedures and
brain reactions. Supply current `Choices`, revision, candidate rows and a live availability
predicate. Candidates are `{id,source,match,decision}` with source `rule/procedure/brain`
and match `exact/ambiguous/none`. There are at most 32, bounded at 65,536 bytes. An exact
candidate needs an action/target pair; the other states may use null.

Stages run in that order. Multiple equal eligible decisions agree; different decisions
at one stage conflict. Ambiguous/conflicting selected stages, stale revisions and no
eligible match return an LLM path. Unavailable or illegal drafts cannot bypass validation.
A higher-priority eligible stage takes precedence over later stages. Replies contain
`route,reason,decision,trace`. Traces are host diagnostics, not model context.

`exact` is a host assertion, not a model confidence estimate. The router does not train
brains, run procedures, interpret text, execute drafts or certify intent. Construct intent-
appropriate choices, retain existing fact/privacy prerequisites and atomically revalidate
at execution. Measure incorrect bypasses as well as calls avoided. There is no automatic
promotion of model answers into reusable rules or approximate decision caching.

## 6. Optional readable-choice responses

`decision::ChoiceCodec::new` accepts existing choices and a complete bijection of
`[{name,pair}]`. Names are unique lowercase identifiers at most 32 bytes; pairs cannot
repeat. `offer()` provides the mapping, `schema()` constrains `{choice:name}`, and `decode`
strictly validates up to 4,096 bytes and returns the original pair. `ChoiceSession` reuses
normal revision/availability/retry/fallback checks with the compact schema in feedback.
For intent guarding, a host can decode then submit the original pair to `IntentSession`;
it must render the appropriate compact schema during corrections too.

Include both offer and schema grounding in request accounting. Use standard or explicitly
compatible instructions: existing action/target guidance does not describe this codec.
Readable names must preserve distinctions and are explicit host configuration. The legacy
format remains the default. Small packets or difficult names may not benefit.

Six one-token probes on the pinned local Qwen 3B Q4_K_M/Ollama 0.40.3 deployment measured
prompt tokens with choices AND schema grounding: 2 choices 158→118, 4 choices 224→174,
8 choices 344→274. These are prompt counts, not generated-response lengths, latency
measurements or accuracy scores. Raw requests/responses: [token probes](../eval/optimization-tokens.json).

## 7. Compare configurations without tuning on holdout outcomes

`tune::compare` accepts `{identity,cases,candidates,limits}`. Cases are independent
`{id,split,expected}` with both `tune` and `holdout` represented. There are 2..512 cases
and 1..16 candidates within a total 1,572,864-byte input cap. Candidates contain
`{profile,recipe,trials}` using existing deployment profiles and complete-turn trial
records. Every candidate must cover every case exactly once, match its independent
expected value and condition name, and pass exact deployment identity matching.

Limits are `min_correct_per_mille` (0..1000), `max_p95_micros` (up to 1e12) and
`max_rss_bytes` (up to 2^54-1). Selection requires tuning correctness, no dropped required
facts, no output truncation, complete input/output token and runner/game RSS metrics,
and the declared latency/RSS limits. Reports retain invalid attempts, retries, fallback
and model-usage coverage from the existing turn evaluator. Pipeline correctness may
include fallback assistance; inspect that separately before adopting a profile.

Only tuning results determine eligibility, the Pareto frontier and the winner. Frontier
objectives are fewer total input+output tokens, lower p95 turn time, lower RSS envelope
and more correct results. The exportable winner is chosen by tokens, latency, RSS,
correctness and input order. Held-out failures remain visible and do not secretly choose
another configuration. A null winner is valid. Repeatedly adjusting after seeing holdout
scores requires a new held-out cohort. This compares recorded configurations; it does not
autogenerate game recipes, run models, verify reported metrics or alter deployments.

The RSS envelope sums separately sampled maximum runner/game RSS; it is conservative
bookkeeping, not simultaneous or uniquely owned physical memory. Other workloads, swap,
thermal limits and available RAM still matter. Missing metrics exclude a tuning candidate
rather than silently treating missing costs as zero.

## Reproduction and limits

`python3 -I eval/optimization_cases.py --check` independently compares Python/Rust outputs.
There are 179 workflows (152 accepted, 27 rejected), 69 new conformance cases, and
30,000 added mutation attempts. All 496 previous cases remain unchanged (565 total).
Persistent Python and native C ABI adapters agree on 358 additional replies/rejections;
these are host bridge checks, not game-engine deployment tests.
`python3 -I eval/optimization_measure.py --cap 0 --check` checks frozen offline measurements.
`python3 -I eval/optimization_measure.py --cap 0 --check-tokens eval/optimization-tokens.json`
audits the saved six token probes without model calls.

The offline fixture packet is 419→49 UTF-8 JSON bytes while retaining item/owner/price.
Typed arguments are 15 bytes versus 56 for the example SQL response; its tool schema costs
206 bytes and must still be counted. These byte examples are not tokenizer savings or a
model-accuracy claim. Eight scripted routing cases produce three local drafts and five
LLM paths; they do not measure real game call reduction. The recorded-cohort scorer keeps
the same tuning winner after an injected held-out failure (held-out 1/2 correct), exposing
rather than optimizing away the failure. Raw evidence: [offline checks](../eval/optimization-measurements.json).

All additions are opt-in and included in v0.3.0a. No API credentials are read, no paid calls are
made, and no larger models are loaded. All evaluation calls require `--cap 0`.

A separate matched-order local accuracy probe used 12 generated scenes, four request
families (ownership, negation, dialogue reference, hostile quoted text), and 2/4/8 choices.
Both formats received the same candidate order, facts and latest request. Conditions were
ordered deterministically at random within each scene, temperature zero, seed 42, one
active CPU request, four requested threads, context 2,048 and output cap 96. No retries or
fallbacks were used in scoring; proposals were decoded and checked against independent
expected pairs. The actual deployment was the same pinned 3.09B Q4_K_M digest used in the
six probes; this was not a larger-model test.

| Format | Correct | Total input tokens | Total output tokens |
|---|---:|---:|---:|
| Action/target | 8/12 | 3,865 | 178 |
| Readable choice | 11/12 | 3,285 | 111 |

Each run first completed 24 one-token reservation checks. These small synthetic scenes
are not live gameplay or a general capability benchmark. The error sets differ, and one
ownership case still fails in both formats. Prompt-cache state is natural. This probe
checks decision correctness and usage; it is not a complete-turn latency or memory study.
The earlier pilot gave 11/12 for both but used different offer orders; its frozen runner
and artifacts are retained and excluded from the matched-format result. The same scenes
were reused to correct the measurement design, without changing codec behavior or tuning
prompts to repair answers. Further claims need fresh scenarios and live integration.

Reproduce with the already-installed model and fresh artifact paths:

```sh
python3 -I eval/optimization_model_eval.py --cap 0 --tokens-only --model arzaan789/qwen2.5-3b-uncensored:latest --output eval/choice-tokens-new.json
python3 -I eval/optimization_model_eval.py --cap 0 --model arzaan789/qwen2.5-3b-uncensored:latest --tokens eval/choice-tokens-new.json --output eval/choice-results-new.json
```

The runner pulls no models and reads no credentials. It permits only loopback local-GGUF
weights. Checks use `--check PATH --cap 0` with no model calls. The matched token and
accuracy runs each spent $0 API; pilot runs and six probes also spent $0. Frozen raw
[matched token probes](../eval/optimization-heldout-tokens.json) and
[matched decisions](../eval/optimization-heldout-results.json) include full requests,
stream chunks and final usage. No new paid-model accuracy claim is made.
