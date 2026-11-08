# Local Qwen deployment check — 2026-11-06

This tests bounded decision turns on the actual four-core, roughly 8 GB deployment.
All calls used installed local weights through Ollama; **total hosted API spend $0**.
No 1B, 1.5B, 8B, 30B or 70B model was loaded or downloaded. Their [workload guidelines](../guides/MODEL_SIZES.md)
are explicitly unmeasured starting experiments, not conclusions from this benchmark.

## Deployment and frozen method

- CPU: Intel Core i7-1065G7, four physical / eight logical cores. MemTotal **7,996,997,632
  bytes**, SwapTotal **2,147,479,552 bytes**. [Hardware record](../../eval/local/local-hardware.json).
- Model: `arzaan789/qwen2.5-3b-uncensored:latest`, 3.09B, Q4_K_M, digest
  `cce688317ea57f96a468e018ed6fe340163484d88ee732410f2ce9f1431f13ec`.
  Runner API version 0.40.3, backend sampled as `llamacpp`, size_vram 0.
- Explicit per-call settings: context **2,048**, threads **4**, CPU requested, temperature
  **0**, seed **42**, decision output cap **96**, safety reserve **256**. This is a starting
  context configuration, not the model's architectural limit. One request at a time;
  weights kept resident. Warmup is separately recorded. No service configuration changed.
- Thirty frozen synthetic scenes, six families × five variations: ownership, negation,
  dialogue/pronouns, current versus old state, affordability computed locally, and hostile
  quoted lore. Independent expectations are generated before inference. Raw offers are
  retained so the affordability oracle is recomputed independently of the visible result.
- Each scene uses identical required facts, current input, two retained exchanges and legal
  pairs in all modes. Raw JSON additionally includes twelve irrelevant/obsolete archive
  events. Scene JSON removes that archive. Scene schema adds a closed whole-pair response
  grammar; all three retain native chat roles and the model's own template.
- Condition order is deterministically shuffled within each scene. Strict JSON, exact
  legal pairs, at most one correction, then a currently allowed fallback/abstention.
  Legal but wrong-intent decisions remain wrong; no oracle repair occurs.
- Timers include native scene hydration/filtering, budget checks, all model calls,
  validation, retry/fallback and sampled telemetry. Fixture generation and precompiled
  request identities are outside timing; hydration is performed again inside the timer.
- Matching token-only counts can be reused. Other requests get a one-token preflight
  **inside** the turn timer. That is additional model work and warms its cache. Natural
  cache reuse is enabled; this is not an isolated cache-disabled prefill comparison.

The measured decision runner is frozen in [ollama_decision_run.py](../../eval/local/ollama_decision_run.py),
matching the recorded runner SHA-256. The current [ollama_eval.py](../../eval/local/ollama_eval.py)
also supports the corrected backend-aware RSS sampler and rejects cloud/nonlocal-weight
metadata. Requests, observations, identity/template, settings and per-call metrics are
retained in [the full decision artifact](../../eval/local/local-model-results.json).

## Token-only first

Thirty one-token probes covered the first six scenes in five conditions; one additional
native-tool probe was run before the follow-up. Both runs cost **$0 API**.

| Initial request | Mean reported prompt tokens / six scenes |
|---|---:|
| Raw archive JSON | 512.5 |
| Scene JSON | 255.5 |
| Scene schema | 255.5 |
| Scene + concise guidance | 287.5 |
| Scene + example guidance | 300.5 |

On the first scene, native tools used **406** prompt tokens, versus **255** for scene
JSON/schema. Schema grammar and native tool schemas do not necessarily enter the prompt
in the same way; count the actual rendering, not HTTP JSON bytes. Every scored generation's
reserved input count matches the actual generation's reported prompt count in the offline
audit. This small corpus stays far below its context limit; post-call counts do not prove
that arbitrary oversized inputs avoid server truncation.

[Main token artifact](../../eval/local/local-model-tokens.json) · [tool token artifact](../../eval/local/local-tools-tokens.json).

## Complete decision turns

| Mode | Pipeline correct / 30 | Correct accepted model decisions / 30 | Retries | Fallbacks | Median turn µs | Nearest-rank p95 µs |
|---|---:|---:|---:|---:|---:|---:|
| Raw archive JSON | 26 | 23 | 7 | 5 | 19,226,226 | 35,123,472 |
| Scene JSON | 30 | 27 | 3 | 3 | 8,651,346 | 19,212,467 |
| Scene schema | 30 | 30 | 0 | 0 | 5,451,699 | 14,292,873 |

A fallback can be correct independently of the model. Raw JSON's five fallbacks include
three correct waits; scene JSON's three fallbacks are all correct waits. The schema result
has no fallback-assisted successes. Raw JSON also accepts two wrong-intent `open` decisions
after initially trying to inspect a warden rather than the gate. Thus fixing shape alone
does not ensure the original instruction survived correction. Two remaining raw failures
repeat the wrong target and fall back to wait. These failures are not rescored as successes.

All modes retain every required fact; no turn loses a required ID. Initial model-correct
counts were 23/30 raw, 27/30 scene JSON and 30/30 scene schema. Invalid attempts were
12/6/0 respectively. There were no decision output truncations in this main cohort.

| Mode | Total input tokens | Total output tokens | Reported cached input tokens | Median first text/tool µs |
|---|---:|---:|---:|---:|
| Raw archive JSON | 35,719 | 1,026 | 24,384 | 13,296,892 |
| Scene JSON | 15,521 | 898 | 13,389 | 2,183,809 |
| Scene schema | 13,867 | 580 | 11,948 | 1,671,903 |

Totals include complete turn calls, **31/27/24** unmatched preflights respectively, and
corrections. First text/tool availability is diagnostic; an action cannot execute until
the complete proposal is validated. Lower input and fewer failures are observed together;
these data do not isolate a universal speedup caused by compression alone.

The original RSS detector did not recognize this installation's `llama-server` backend,
so main-cohort runner RSS correctly remains **null**. Host Python RSS maxima were
26,341,376 / 27,947,008 / 26,300,416 bytes. System available RAM minima were
1,163,890,688 / 1,322,430,464 / 1,154,347,008 bytes. Swap activity occurred; for example,
raw turns reached 97,914,880 bytes swap-in and 370,368,512 bytes swap-out within a sampled
turn. These are shared-host observations, not uniquely attributed model allocations.
Background memory pressure and natural cache reuse affect absolute/tail latency.

## Guidance, native tools and narration follow-up

A separate six-scene pilot tested concise guidance, example guidance and native tool
transport, all with a subsequent bounded narration call. It cost **$0 API**. It is not
latency-comparable to the decision-only table above because it includes narration and
uses a corrected RSS sampler. The baseline schema already answered all these six cases.

| Pilot condition | Decision correct / 6 | Retries / fallbacks | Median complete-turn µs | Sampled runner RSS max bytes |
|---|---:|---:|---:|---:|
| Concise guidance + narration | 6 | 0 / 0 | 15,173,285 | 2,705,707,008 |
| Example guidance + narration | 6 | 0 / 0 | 11,781,427 | 2,705,190,912 |
| Native tools + narration | 5 | 1 / 1 | 23,026,300 | 2,705,707,008 |

One native-tool output truncated; its correction failed and the turn fell back to wait
when opening was requested. Advertised tool capability alone is not evidence of adequate
end-to-end performance. Extra guidance added tokens without a demonstrated accuracy gain
on this small subset, so standard supplemental guidance remains empty and transport is
explicit. No universal model-size choice is inferred.

The corrected sampler identifies inference processes by their ancestry under Ollama,
including `llama-server`, excluding unrelated standalone servers. The observed maximum
is about **2.52 GiB** RSS. Some follow-up trials had only **689,397,760 bytes** available
system RAM and substantial swap activity. Maxima are sampled between chunks, not allocator
peaks/PSS; native CLI child peaks are excluded. Narrative responses are saved for review,
but all dialogue quality scores remain null: correct actions do not prove grounded prose.

[Full follow-up artifact](../../eval/local/local-guidance-results.json).

## Compatibility, checks and limitations

- Independent Python/Rust comparison: **148** workflows, **132 accepted / 16 rejected**,
  including 100 generated scene cases. **48** new conformance cases; all **448** prior
  cases remain byte-identical. Adds **20,000** deterministic parser mutations.
- Language transports additionally agree on **296** local Python/native workflows and
  **48** C# fixture replies (plus two protocol smoke checks). No model calls are made by
  these checks.
- Existing kit v1 evaluator is unchanged. Full-turn telemetry has a separate contract;
  unknown metrics remain null. The offline audit replays native choices/reporting, checks
  complete cohorts, independent offers, actual required facts and prompt reservations.
- The model received only small synthetic tasks with short IDs and a supplied legal-pair
  list. There is one exchange per scene/condition, not broad multi-turn gameplay testing,
  general 1.5B capability evidence, or a statistical 100% reliability guarantee. Thirty
  samples give an initial tail estimate; larger/live independent-template cohorts remain.
- Native tools, concise/example guidance and narration have only six-scene pilot coverage.
  Signature/plain transport paths are available for future evaluation, not measured here.
- `eval/local/local-model-pilot.json` retains nine earlier completed pilot turns plus their
  observations. Its timer began after hydration, so it is excluded from complete-turn
  comparisons. An interrupted call has no completed metrics. No paid usage was incurred
  and no prompt/answer tuning was selected from its scores.
- Current local-weight checks reject cloud model metadata, but the local server and host
  counter are trusted. This library does not install tokenizers, certify backend claims
  or change Ollama environment/cache settings.

Offline audit (no model invocation): run `eval/local/test_ollama.py`,
`eval/local/local_cases.py --check`, and `eval/local/check_local_model.py --cap 0` with the four main
artifacts above. CI runs these without credentials or Ollama. Live repetitions require a
running installed local GGUF model, explicit `--cap 0`, a fresh output path, token-only
checks first, then paired accuracy conditions. Existing Venice cohorts remain frozen.
