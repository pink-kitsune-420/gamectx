# Roadmap

One line per item. Numbers are from `eval/` on Llama 3.2 3B and Mistral Small 3.2 24B unless
noted; "3 models" adds Llama 3.3 70B.

## Shipped (0.1.0-alpha.2)

| Item | Result |
|---|---|
| Tables, `group_by` | −38% input tokens vs compact JSON, 140/159 vs 133/159 correct (3 models) |
| Nested `group_by` | two-column filters 7/12 → 12/12, −2 to −3% tokens |
| `signatures` | −12 to −14% vs packed JSON Schema; −46% vs JSON overall, 141/159 (3 models) |
| `relevant_to` (tables) | −26% tokens, identical accuracy (100/106 both) |
| `relevant_fallback: "none"` | another −59% tokens; loses only whole-table questions |
| `where`, `derive`, formulas | direct arithmetic 0/16, model-written expression via `calc` 8/16; misses were row selection |
| `memory` recall | −84% tokens vs a 60-event history, 20/20 both |
| `round` view | about one token per dropped digit per number (−5% for one place here) |

## Implemented (unreleased)

| Item | Result |
|---|---|
| Model-written `query` tool (bounded SELECT, filters, sorting, aggregates) | 29 questions × 2 small models: direct 33/58 → SQL 53/58; numeric filters 0/6 → 6/6; arithmetic 1/16 → 12/16; packed tool 52/58 in a separate run. Full tables still sent. 116 recorded queries and 3,000 generated/mutated queries byte-identical to Python; 36 new conformance cases, 30,000 fuzz mutations. |
| Query token-only check and eval cap enforcement | Direct vs tool: 1,471 → 1,611 input tokens (Llama), 1,490 → 1,626 (Mistral); all four omitted cached-token accounting. Cost $0.000756. |
| Schema-only descriptions, explicit weight fields and bounded correction sessions | On 3 held-out states (24/48/96 inventory rows, 10/20/40 NPCs), full tables 160/174 first → 171/174 after retry; schemas 169/174 → 172/174. Final answer scores identical to executed-query scores. Complete-exchange average input 4,898.632 → 1,615.874 (−67.0%); completed-exchange cost $0.107952 → $0.038804 (−64.1%). 23 new cases; 547 measured attempt envelopes byte-identical to Python. |
| Generic adaptive local reaction brains | Offline reference-policy agreement on four tasks: random 545/1,536 → trained 1,421/1,536 (fresh seeds 707/808/909); changed preference 264/384 → 384/384 (seeds 1001/1002/1003). Graphs, training, pins, validation gates, persistence/undo; 41 new cases, 30,000 text mutations, 292 byte-identical workflows including 2,304 predictions. No API calls. |
| Opt-in validation-reaction rate selection | Fresh paired seeds 2001/2002/2003: single rate 1,436/1,536 → selected 1,487/1,536; nonlinear task 315/384 → 359/384. Rates chosen only on validation data; about 3× estimated training work, identical inference. 17 new cases, 45 byte-identical selection workflows/1,536 predictions; 10,000 additional text mutations. No API calls. |
| Preloaded templates and opt-in bounded prediction caching | Local release medians, five trials: 20,000 repeated-state predictions 15.939–19.223 ms cached vs 29.161–36.524 ms uncached (1.7–2.0×); cloning 1,000 templates 1.412/8.952 ms vs JSON restore 24.238/142.615 ms (small/large). Independent owners, 0–256 FIFO entries, mutation invalidation; six new cases, 64 byte-identical cached workflows and 10,000 workflow mutations. No API calls. |
| Native frame inference and faster packet assembly | 20,000 changing-state calls: JSON 26,030,769/32,503,158 ns → native 1,968,370/6,171,925 ns (small/large), zero native allocations. 500 pressured packet builds 146,774,066 → 19,042,995 ns; prepared reuse 1,834,731 ns. Exact output/receipt parity. |
| Scheduling, sliced training and host-policy validation | Synthetic 64 actors/240 frames: 15,360 → 512 evaluations, max eight/frame. Sample-sliced train/selection preserves weights/reports/state; cancellation and stale-state rejection. Host masks/thresholds evaluated on fresh seeds 3001/3002/3003: 1,466/1,536 for both old and host-metric selection, no measured gain. |
| Acknowledged context synchronization and local prefix harness | 120 turns: 489,490 full-state bytes → 19,089 transfer bytes, exact reconstruction; explicit ACK, stream/revision checks and snapshot recovery. 35 new cases across runtime APIs, 148 byte-identical workflows/2,304 predictions, 40,000 extra text mutations. All offline, $0; actual local-model KV-cache savings unmeasured. |

## Measured and rejected

| Idea | Why not |
|---|---|
| Symbol/lookup indexes | no token saving; accuracy 102 → 74 of 108 |
| Plain-text (CSV-like) tables | 0 to +1% tokens: `","` is already one token |
| Row counts in table headers | +2/54 (noise) for +3% tokens |
| Sorting rows to help filtering | no accuracy change on its own |
| All-text packet instead of JSON | only ~1% better than signatures inside JSON |
| JSON query interface alongside SQL | 52/58 vs SQL 53/58 (noise), but more input tokens on both models: 1,601/1,616 vs 1,569/1,586; invented `group_by` filters. Keep SQL as the one shipped query interface. |
| Linear-only default reaction network | On the nonlinear switch holdout, 192/384 versus 306/384 with four hidden nodes; retain linear networks as an explicit option. |
| Straight-through gradients at every brain node | Diagnostic seeds 101/202/303: 1,243/1,536 versus 1,430/1,536 with output-only straight-through gradients. Strict clamp gradients scored 1,424/1,536 but failed the saturated-output adaptation test; use output-only recovery. |
| More examples alone as a universal brain-accuracy fix | At equal 4,096 training sample-visits, switch validation MSE improved 0.012898 → 0.009566 but correct reactions fell 110/128 → 103/128 (32 → 128 training examples). More data can help other tasks; decision quality needs its own check. |
| One universal initializer/rate | Switch validation scores ranged 74/128 (initializer 1) to 114/128 (7), versus 110/128 (3). Rate 0.3 helped switch (119/128) but hurt support (116/128 vs 119/128 at 0.1); use bounded validation selection without resetting an existing brain's learned state. |
| Always-on brain prediction caching | Cycling 256 distinct states through 32 slots: 0 hits/20,000 calls; small network 29.011 ms uncached → 53.640 ms cached, large 37.100 → 67.276 ms (1.8–1.85× slower). Keep caching opt-in with counters; no approximate input bucketing added. |
| Universal one-shot packet speedup | The fitting fixture took 5,587,160 → 6,098,296 ns (about 9% slower) despite allocations falling 47,000 → 30,500. The initial always-prepared variant added allocations too; the shipped fit path moves values directly. Prefer prepared reuse for repeated builds, and measure one-shot tradeoffs. |
| Host-policy selection as an automatic accuracy boost | Same rates and 1,466/1,536 held-out masked/thresholded decisions as prior selection. Ship as opt-in metric correctness, not a measured accuracy win. |

## Next candidates

- Catalog `relevant_to` measured with model questions (only checked by conformance cases so far).
- Clearer top-N aggregate guidance: Mistral still generates subqueries after a correction in some cases.
- Explicit categorical semantics (e.g. rarity label `unique` versus count = 1), tested on fresh holdout seeds.
- Reaction-brain feedback from real hosts and unseen policy families; native input adapters and held-out gameplay metrics remain integration work; the generic scheduler needs host notifications/costs.
- Measure acknowledged synchronization and periodic snapshot policies on live multi-turn model decisions.
- Provider prompt-cache discounts; the small-model query token check omitted cached-token accounting, so discounts remain unverified.
- Ports (TypeScript first, then C#/Python) checked against `conformance/`.
- Feedback from the first real game integration once its gaps list arrives.

## Known gaps

- Word matching is literal plus an optional plural `s`: "lighting" does not match "light".
- Numeric filtering by reading rows fails in every format; use `where` or have the model request a `query`.
- Query is a SQL subset: one table, AND/IN, no joins, subqueries or GROUP BY; ORDER BY/LIMIT run before aggregates. Arithmetic uses 64-bit floats.
- Schema-only requests can confuse semantic labels (`unique` rarity became count = 1). Valid but wrong queries do not retry.
- Query workflow eval covers 4 states of one synthetic game family, including 3 holdouts; real games and different distributions may differ.
- Brain tests use synthetic labels and one initializer in final test runs, not live gameplay or model-authored-policy quality. Original single-rate switch test misses 78/384; rate selection misses 25/384 on a fresh cohort (paired baseline misses 69/384). Validation improvements do not guarantee gameplay or out-of-distribution behavior.
- Default rate selection measures decisions with every output eligible and zero thresholds. Hosts can opt into `train_select_host` for masks/thresholds, but selection still adds training work and can overfit reused validation data.

- Runtime timing fixtures use two synthetic network sizes and mostly static packet reuse; real engines, allocators and actor change rates may differ. Estimated frame-work budgets are not wall-clock deadlines.
- Sync deltas compress host transfer, not automatically LLM input/history; integrations reconstruct snapshots or implement explicit state handling. Actual local prefix-cache tokens/timings remain unmeasured without a running server.
