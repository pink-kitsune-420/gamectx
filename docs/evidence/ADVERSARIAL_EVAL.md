# Adversarial probing and measured limits (unreleased v0.3.1a)

This work tests host correctness separately from model judgment. No historical model
results or existing conformance expectations were replaced. Model/API spend: **$0**;
**159 local model calls** are retained, including token-only checks and preflights.
No other model was loaded/downloaded, and no runner/service settings were changed.

## Reproduced host defects and repairs

`eval/local/adversarial_probe.py --cap 0 --require-fixed` exercises 18 conversation commands
across three seeds, plus one capped-history and two restore checks. Baseline: **15/57**
pass, **42/57** fail. After repairs: **57/57** pass. The same run compares **900** engine
steps with the independent Python game (150 per seed: 0, 7, 101, 303, 409, 65535). All
900 engine responses and states match, including rejected illegal/stale actions.
These are synthetic targeted tests, not general language understanding.

- The conversation shortcut previously executed 13 negated, conditional, quoted or
  compound commands per seed. `take no bronze key`, `go nowhere near gallery`, and a
  wholly quoted `take` command could act. The engine correctly enforced legality, but
  those legal actions did not match the user's intent. The shortcut now accepts only
  a whole explicit command/alias, with bounded articles, prepositions and politeness.
  Unsupported clauses/questions ask for clarification. Exact listed commands still work.
- A live older report could contradict three newer reports yet disappear from a capped
  time summary's conflict flag. Conflict detection now examines all usable authorized
  records in the supplied view, while retaining the configured output cap. Same-calendar
  interval bounds allow a linear scan, using `max(low)>min(high)`. No winner is invented.
  Explicit task selection still determines its own view; excluded sources are not searched.
- Restore accepted a future activity marked completed before its window, and a changed
  condition list under the same revision. It now checks non-planned lifecycle timestamps,
  interruption/cancellation policy consistency and exact same-revision frame identity.
  Valid save/restore remains supported; host storage/authentication and durable replay
  watermarks remain necessary. These are privileged host inputs, not model-callable APIs.

Two additional **fake-transport** probes found that neutral request data could override
`LocalModel`'s pinned model, and that a single remaining call could be spent on a preflight
without enough allowance to generate. Neither probe contacted a model. Model/transport
controls are now rejected in neutral requests; whole-turn call allowance is checked
before inference. Nonpositive/invalid output reservations fail before dispatch.

Evidence: [baseline](../../eval/local/adversarial-baseline.json),
[fixed probes](../../eval/local/adversarial-fixed.json),
[adapter baseline](../../eval/local/local-adapter-baseline.json).
Their HEAD fields record the then-current commit; the fixed run used the working patch.
The independent Python guards, 96 time-conflict workflows, 162 literal-command comparisons
and 64 typed-assessment workflows are in `adversarial_reference.py`/`adversarial_cases.py`.
Ten thousand new command mutations supplement the existing Rust parser/restore fuzzing.
Fifteen additive core cases preserve all **827** earlier cases; total **842**.

## What the installed model got wrong

Model: installed Qwen2.5-derived **3.09B Q4_K_M**, digest
`cce688317ea57f96a468e018ed6fe340163484d88ee732410f2ce9f1431f13ec`;
Ollama **0.40.3**, four CPU threads, GPU **0**, context **2,048**, temperature **0**,
seed **42**, output reservation **32**, safety margin **256**. Full template, hardware
and post-probe read-only metadata: [environment](../../eval/local/character-limit-environment.json).
This machine has **4 physical / 8 logical cores** and **7,996,997,632 bytes RAM**.
No conclusion about all models of this size follows from this one variant.

Sixteen fresh source/time states each receive focused and expanded requests. Cases cover
no source, planned-only walks, sun through a window, blocked/cloudy views, aging/expired
observations, wrong/broken watches, approximate routines, conflicting reports, disabled
entitlement, quoted instructions and misleading old dialogue. Both formats retain the
same required host-approved time facts. Expanded adds raw observations, routine and old
conversation. The task selects one of five source categories, not unrestricted speech.
Gold categories follow explicit host rules; valid JSON is scored separately. Source
descriptions can be terse or distract from the structured metadata; this tests the
provided contract, not whether a sensor description logically proves its reading.

A completed **32-call token-only run** precedes generation. Every generation turn then
includes another actual preflight of its full request. Maximum observed prompt: **1,150
tokens**, below the 2,048-token context after output/safety reservation. The schema is
in the request as well as the native output grammar; provider-reported counts include
the template/messages. There are no tool rounds hidden from these totals.

| Condition | Correct | Valid JSON | Model calls | Complete-turn input tokens | Median turn (microseconds) |
|---|---:|---:|---:|---:|---:|
| Focused | 12/16 | 16/16 | 32 | 6,440 | 1,847,744.5 |
| Expanded | 13/16 | 16/16 | 32 | 19,460 | 6,303,493 |

There are **zero retries** and **zero truncated generations**. The earlier token-only
calls are additional, not included in the generation-turn columns. Timing includes
packet-to-request assembly, memory sampling, actual preflight, generation and scoring;
the engine snapshot is captured/frozen outside the timer. These are complete diagnostic
turns over fixed packets, not end-to-end live dungeon turns or cold-load comparisons.
Natural prefix caching is uncontrolled; cached counts are retained in raw responses.

Both modes mislabeled visible/aging sun estimates as reports. Both report-quotation
cases returned the embedded instruction's requested `exact` despite metadata requiring
`reported`. There is no paired neutral-quotation control, so this does **not** establish
causality. Focused mode also mislabeled one exact instrument reading as reported. The
seven misses demonstrate that JSON grammar and shorter context do not establish source
following. The smaller packet is faster here but scores one answer worse; it is not
accepted as an accuracy improvement.

Sampled runner RSS peaks at **2,558,652,416 bytes** across this classification run.
Harness RSS peaks at **27,287,552 bytes**; it is not the Rust engine's RSS. Minimum sampled
system available RAM is **2,558,341,120 bytes**. Global swap activity was observed and is
retained; it cannot be attributed to either format under uncontrolled desktop load.
RSS is sampled between response chunks, not an allocator peak or a model-memory guarantee.

Raw evidence: [tokens](../../eval/local/character-limit-tokens.json),
[results](../../eval/local/character-limit-results.json),
[recomputed summary](../../eval/local/character-limit-summary.json).
`check_character_limits.py --cap 0 --check eval/local/character-limit-summary.json` audits the
requests, identities, counts, scores and all five local run artifacts without a model.

## Deterministic time assessment instead of model classification

Optional `History::assess_time` / `Runtime::time_assessment` returns a sealed
`TimeAssessment`, with `TimeKind::{Unknown,Approximate,ExactReading,Reported,Conflicting}`
and the original knowledge payload. It follows host source metadata without reading
quoted text as instructions. The host can localize/render an answer or select an authored
answer card. ExactReading means an instrument's displayed reading; it may be wrong in the
game. Reported means attributed claims, not verified current time. Existing outputs,
tool registrations and default exposures stay unchanged. No extra model call is needed.

The new host bridge read is `character_routine_assess_time` with
`id,revision,max_bytes`; Python/Node helpers are available, and C#/C retain the session
ABI. It cannot be called through model dispatch. Sixty-four additional synthetic states
match an independent kind oracle; ten core conformance cases include four errors.
This validates a deterministic contract, not improved language-model accuracy.

## Optional reuse of verified full-request token counts

Run the dungeon with `--ai --count-cache 8`, or construct Python `LocalModel` with
`count_cache=8`. Default **0** disables reuse; capacity is **0..64** entries. Only token
counts are cached. Generated answers, world actions and observations are never cached.

A cache miss runs the normal full-request preflight and generation, storing a count only
when both successful calls agree. The key covers the complete neutral request (messages,
tools, grammar, history/results/corrections), output reservation, context and verified
model/template/runner identity. Changing any of those recounts. Every generation still
checks identity and the returned input count. A mismatch discards the entry and rejects
the reply. Bounded receipts cite the original preflight/generation; FIFO limits retention.
One remaining call can serve a verified hit; a miss needs allowance for two. This is an
example Ollama adapter optimization, not a universal provider-tokenizer cache.

Three fixed requests, three repetitions each, alternate baseline/cached order. First use
misses; subsequent identical requests reuse counts. All paired answers are identical.
Both arms score only **3/9**; this is a performance workload reused from the earlier
cohort, **not** a new accuracy test. It uses the nonstreaming UI adapter and natural cache
state, so its answers are not a controlled consistency comparison with the streaming run.

| Production adapter | Model calls / 9 turns | Input tokens | Median all turns (µs) | Median six repeated turns (µs) |
|---|---:|---:|---:|---:|
| Fresh preflight each turn | 18 | 3,630 | 2,023,613 | 1,780,275 |
| Verified count reuse | 12 | 2,420 | 1,301,517 | 1,637,055 |

The production timer includes packet/request construction, copying, identity checks,
all model calls, scoring and memory sampling. Engine capture is outside this diagnostic
clock. Initial construction of the adapter/model metadata is also outside. Nine turns
per arm and six repeats do not establish tail latency or broad gameplay speedups. Dynamic
prompts often miss this exact-match cache. No context/tool capabilities grow automatically.

The earlier independent prototype pilot is retained in
[count-cache-results.json](../../eval/query/count-cache-results.json). Its adapter-only timer
excluded scoring/memory sampling and is not reported as complete-turn latency. Production
evidence: [token prerequisite](../../eval/query/count-cache-tokens.json),
[production results](../../eval/query/count-cache-production.json). Token checks use **3** calls,
the prototype **30**, production **30**; classification uses **96**, totaling **159**.
Every run API spend **$0**. No hosted allowance was used.

## Remaining limits

This does not establish general NPC intelligence, multilingual intent handling, speech
truthfulness, long-session gameplay reliability, cold latency or memory under engine load.
A host should do exact arithmetic, eligibility, source classification and permissions
locally. Keep a model on a bounded task that benefits from language; validate and confirm
engine actions. Broader models/providers, context sizes and cache settings need their own
measurements. Existing failed speech pilots and historical model results remain intact.

Validation completed: root/native formatting, strict Clippy, tests/rustdoc, Rust 1.82,
82 offline CI commands, 21 binding CI commands, all earlier and 61 expanded C# fixtures,
package verification and the reference/credential sweep. All 827 previous conformance
cases remain byte-identical; 15 additions bring the total to 842. No push/tag/publication.

The later [dynamic-character diagnostic](LIFE_EVAL.md) retains four new speech families,
24 local calls and $0 API spend. All eight replies have valid shapes; both arms invent
birthplace details, and character context also invents weather and yields unfinished
replies. This does not replace or repair the historical diagnostics above. Stable prefix
ordering and deterministic state checks establish no semantic grounding guarantee.
