# Size-and-task workflows (v0.3.0a)

Give a small model one immediate job: interpret a request, speak as one character,
narrate an approved scene, or write one bounded query. Resolve visibility, arithmetic,
ownership, prices and world changes locally. These additions preserve all older packet,
profile and chat-renderer formats. They are included in v0.3.0a.

## Editable profiles

`deployment::ModelSize::workflow_profile(Workload, name, identity)` adds `workflow`
metadata alongside the existing `guideline` and v1 `profile`. Bridge/CLI local kind
`workflow` takes `{size,task,name,identity}`. The 24 fixture profiles are in
[`examples/fixtures/deployments/workflows.json`](../../examples/fixtures/deployments/workflows.json).
[`examples/packet/task-workflow.rs`](../../examples/packet/task-workflow.rs) is executable and tested.

| Size class | Context start | Interpret output | Dialogue output / exchanges / recall | Narration output | Query output |
|---|---:|---:|---:|---:|---:|
| 1B | 2048 | 32 | 64 / 1 / 1 | 64 | 64 |
| 1.5B | 2048 | 32 | 96 / 1 / 1 | 96 | 96 |
| 3B | 2048 | 32 | 128 / 1 / 2 | 128 | 128 |
| 8B | 4096 | 32 | 160 / 2 / 3 | 192 | 160 |
| 30B | 8192 | 32 | 192 / 3 / 4 | 256 | 192 |
| 70B | 8192 | 32 | 192 / 3 / 6 | 256 | 256 |

All are **unmeasured generic starting hints**, including the 3B tier. The exact local
Qwen deployment below has a small workload measurement; it does not establish an optimal
configuration for every 3B model. Context is a starting budget, not an architectural
limit. Larger tiers imply neither a need for larger packets nor that weights fit 8 GB RAM.
The existing generic `guideline.output_start` stays unchanged; task-specific output is
in `profile.budget.output`. Safety remains 256. Choices hints remain 4/6/8/16/32/32.

The generator does not load models, select content, silently shorten dialogue or apply
configuration. Hosts can override every hint, preserve required facts and required recent
exchanges, and fail or clarify if they cannot fit. In the example, a choice-count hint
causes clarification instead of silently deleting candidates. Do not use a task profile
as a measured capability boundary. Pin actual model/quantization/template/tokenizer/runner
identity before evaluating or adopting a configuration.

## Direct current requests

`dialogue::render_direct` is an opt-in alternative to `render_chat`. It emits native
system/user/assistant messages, keeps whole supplied dialogue verbatim, and ends the final
user message with `Current request:` followed by the current text. Context data precedes
it as compact JSON. It does not insert ChatML, tools, a response schema or a tokenizer.
The current utterance no longer sits inside several quoted JSON objects.

The input is a bounded object with `rules`, `dialogue` and `input:{id,text}`. Dialogue
must be at most 256 messages, alternate user/assistant, and contain complete pairs.
All other fields become context data in insertion order. Current ID and text are strings;
message objects contain exactly role/content. Input is bounded to 1,572,864 bytes.
The existing renderer and all previous expected outputs remain unchanged. Thirty new
conformance cases cover 24 profiles, two renders and four errors; all 565 prior cases
remain byte-identical. Sixty persistent/native bridge checks match the same reference.
Count the **complete final provider rendering**, including tools,
schema and template, and reserve output plus safety; JSON byte estimates are not exact.

## Ambiguous entity references

The shared `dialogue::resolve_references` now provides the underlying lexical resolver;
[CONVERSATION](CONVERSATION.md) specifies its language-configured contract. The dungeon's
[`intent.rs`](../../examples/lantern-vault/intent.rs) supplies English action/type policy. This is
not a universal semantic language parser.
It starts from current actor-visible entities and currently legal pairs:

1. Match complete token phrases from IDs, labels and item kinds. Suppress a shorter
   alias contained within a longer phrase: “keeper seal” must not also mean “keeper”.
2. Where explicit canonical verbs establish a target type, use it: talking selects an
   NPC, taking selects an item. Never manufacture a missing or forbidden action.
3. Resolve “it” from a recent accepted action only while its target is still available,
   in the same room, and no more than four accepted turns old. A room change or finished
   conversation clears this action focus. Quoted model prose never establishes focus.
4. Multiple remaining matches or missing pronoun context produce named candidates and
   a targeted clarification. A human-selected ID is revision-bound and checked again
   against live targets before interpreting the action. Never select the first match.

Negation and complex requests still require understanding; a substring heuristic is not
semantic proof. Unknown wording can reach the model, which may still choose a legal but
wrong action. The interactive player confirms model-proposed world changes. Explicit
listed commands bypass generation. Local exact guards and clarification should be
measured separately from model accuracy, including incorrect bypasses.

## Local measurements and remaining failures

Deployment: the same installed Qwen 3.09B Q4_K_M model and pinned Ollama 0.40.3 identity
as the earlier local report; CPU-only, four threads, context 2048, temperature 0, seed 42.
Each cohort has nine character replies and four intent requests, preceded by a matching
four-probe token run. Every generation also has a one-token native preflight. Each
accuracy cohort used 26 calls, no correction, and **$0 API spend**; every token run also
spent $0. The smaller table below counts generation requests only, excluding preflights.

| Workflow | Merchant / keeper / narrator initial input | Generation input / output tokens | Intent correct |
|---|---:|---:|---:|
| Standard | 531 / 452 / 368 | 7468 / 525 | 2/4 |
| Extra grounded instructions | 614 / 535 / 451 | 8240 / 610 | 1/4 |
| Focused task and direct renderer | 508 / 381 / 286 | 6767 / 308 | 2/4 |

The focused variant changes several things together: goal removal for interpretation,
shorter structured output, item-capability facts and smaller dialogue selection. These
numbers do not isolate a renderer effect. Extra instructions did not solve the failures
and remain an explicit comparison option. Keep all three frozen cohorts, including
wrong answers. Required facts must not be removed to reproduce a token number.

An exact-slot pilot used the same four requests plus four new phrasings. It scored 7/8:
three local deferrals and 4/5 correct model decisions, ten total preflight/generation calls.
After fixing overlapping entity phrases, the same eight requests scored 8/8: four local
deferrals and 4/4 model decisions, eight calls. The initial token runs used five and four
probes respectively; all four runs spent $0 API. The revised eight cases are a regression
pilot, **not an independent held-out generalization claim**. Independent Python/Rust
resolver checks cover 360 workflows across six seeds; those are offline rule checks,
not unseen-state model accuracy. Synthetic duplicate-key, stale-selection, focus and
clarification tests cover additional failure boundaries.

NPC speech still sometimes contradicts supplied ownership or invents tonic effects.
Remembering a name does not make all dialogue accurate. Citation validation establishes
source availability, not entailment. Speech has no action channel and changes no world
state; focused speech has no claimed citations. Do not trust its prose as authorization.

These short sequential diagnostics are not controlled complete-turn latency or memory
benchmarks. Calls retain individual elapsed time and native usage, but runner RSS is not
sampled, and complete-turn setup/selection costs are not recorded by these new scripts.
Use the existing [local evaluation harness](LOCAL.md) for latency/retry/memory studies,
then add fresh seeds and semantic families before promoting any profile to measured.
Native preflight counts are post-call observations, not proof arbitrary oversized input
escaped runner truncation. The host first bounds and estimates the request; explicit
required-fit failures prevent guessing after a known budget failure.

Offline reproduction (no model required):

```sh
cargo build --locked
cargo build --example dungeon --locked
python3 -I eval/local/workflow_cases.py --cap 0 --check
python3 -I eval/lantern-vault/dungeon_social_cases.py --cap 0 --check
python3 -I eval/lantern-vault/test_dungeon_ai.py
python3 -I eval/lantern-vault/check_dungeon_slots.py --cap 0 --check eval/lantern-vault/dungeon-slots-tokens.json eval/lantern-vault/dungeon-slots-results.json eval/lantern-vault/dungeon-entities-tokens.json eval/lantern-vault/dungeon-entities-results.json
python3 -I eval/lantern-vault/check_dungeon_social.py --cap 0 eval/lantern-vault/dungeon-social-tokens.json eval/lantern-vault/dungeon-social-results.json eval/lantern-vault/dungeon-social-guided-tokens.json eval/lantern-vault/dungeon-social-guided-results.json eval/lantern-vault/dungeon-social-focused-tokens.json eval/lantern-vault/dungeon-social-focused-results.json
```
