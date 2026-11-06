# Dialogue routing and perception checks — 2026-11-06

This unreleased v0.3.1a work separates host routing, access control and model speech.
All new API spend is **$0**. No hosted model or larger local model was used.

## Local correctness, not model accuracy

`eval/conversation_patch_cases.py --cap 0` compares the previous v7 example policy with
an independent Python prototype and Rust on 13 reported question types × six seeds.
Exact intended candidate sets improve **0/78 → 78/78**. This strict route metric is
not an answer-accuracy score: a broader prior candidate set fails even if it includes
an acceptable answer. The fixtures reuse reported phrasing and vary state; they do not
prove generalization to new phrasing or game families.

Identity, item availability, stock follow-ups, topic menus, scene/world questions,
time uncertainty, unknown real-world history, player weather reports, subjective sky
questions and literal character counts are resolved locally when unambiguous. A scripted
sequence additionally checks actual key transfer, its use, strawberry **10**, cake **4**,
attributed name recall and scene lookup. It makes no model calls. Player input now uses
`you>` instead of the NPC's name. The new fallback is in character, and player weather
reports cannot become observed weather through these closed routes.

`conversation_time_cases.py` adds **14** workflows for no clock, clock/watch entitlement,
schedule estimates, withheld configuration, reported time and forgetting, across both
lore versions. The merchant defaults to no instrument; the keeper estimates a routine
phase. Schedule knowledge does not disclose the precise internal clock.

New independent Python/Rust byte comparisons:

| Area | Workflows | New core conformance fixtures |
|---|---:|---:|
| Location and attributed quote phrasing | 58 | 26 |
| Host-authorized world states | 63 | 15 |
| Task/context selection | 49 | 9 |

The **50** new core fixtures are additive; the prior **747** remain unchanged.
Mutation testing adds **20,000** perception/character text attempts. Persistent
character checks add **30** fixtures through Python CLI, Python native, Node and C#
native; helper constructors/calls are checked separately. Sources and revisions remain
host-bound. These checks establish behavior and byte parity, not live tool-selection
accuracy, tokenizer savings or semantic entailment.

The synthetic context fixture keeps **1,025 bytes** under focused limits and **3,733
bytes** under expanded limits; raising the context budget alone leaves focused limits
unchanged. Its one-byte counter is deliberately synthetic. Real deployments must count
the full provider-rendered request, including tool formatting and output reservation.

## Retained unsuccessful model diagnostics

Installed model: local Qwen2.5-derived **3.09B Q4_K_M**, digest recorded in each artifact,
Ollama **0.40.3**, CPU **four threads**, GPU **0**, context **2,048**, temperature **0**,
seed **42**. This is the installed variant, not a claim about all Qwen2.5 deployments.
Each variant ran two token-only checks before two generation cases; the generation
cases each include another preflight. Each variant therefore records **six local calls**
across its token and result files, **18 calls** total. Recorded cached-token counts mean
these are not controlled cold-latency comparisons. RAM was not measured in these diagnostics.

| Prototype | Reported input tokens per question | Result |
|---|---:|---|
| Broad guarded facts | 643 | Sky reply invented clear blue weather; underground reply invented a personal history and dampness. |
| Compact subjective prompt v2 | 225 | Both replies stopped mid-sentence at the 160-character schema bound. |
| Compact one-sentence grammar v3 | 241 | Complete subjective feeling reply; sky reply still implied a current clear blue sky despite the imagination prefix. |

Artifacts: `eval/conversation-patch-{tokens,results}.json`,
`eval/opinion-v2-{tokens,results}.json`, `eval/opinion-v3-{tokens,results}.json`.
`check_conversation_patch.py --cap 0` audits all **12** recorded case records offline,
including request reconstruction, token-first identity, CPU/context settings, call
counts and raw responses. The broad prototype remains reproducible through the explicit
speech host operation, but the default sky question uses a bounded authored response.

The compact prompts reduce reported input but do not establish a safe, fluid replacement.
Neither shorter context, a second phrasing pass nor a sentence-prefix grammar is accepted
as a semantic guarantee. These failed experiments are retained in ROADMAP. General
free-form speech remains experimental and can invent facts outside the closed routes.

## Next evaluation

Test new question templates and scenes without tuning on their answers. Separately
measure local routing mistakes, tool selection, factual correctness/attribution, complete
turn latency including preflights/corrections, retries, runner RSS and system RAM. Compare
focused versus expanded attention under the same full-request budget and model identity.
Include clocks that are wrong, schedule drift, cloudy skies, stale reports, changed
visibility and conflicting speakers. Keep costs capped; run token-only checks before
any new local or hosted accuracy batch.

Validation completed: root/native formatting and strict Clippy, Rust tests/rustdoc,
source/native tests on Rust 1.82, package verification, 74 offline CI commands and 15
binding CI commands. C# checks pass for 30 new actor fixtures plus the previous 126
information, 59 kit and 48 local fixtures. All 747 previous conformance cases remain
byte-identical; total core cases are 797. Reference sweep is clean. No new release has
been published by these checks.
