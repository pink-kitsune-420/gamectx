# Alpha.3 model test — 2026-11-06

Fresh synthetic cargo, team and route states, two small hosted models, and three context/tool formats. This tests packing and querying; local brains, memory reasoning, scheduling and live gameplay remain separate evaluations.

## Completed exchanges

Table metrics include every query attempt, error correction and final-answer call within each completed exchange. The single interrupted runner call is separately charged in total spending below. Direct JSON and packed tables use one call. Query mode executes the released Rust tool, permits one syntax/validation correction, and asks the model for a final answer.

| Format | Correct / 96 | Average total input tokens | Average total output tokens | Usage-priced cost |
|---|---:|---:|---:|---:|
| Full JSON | 27/96 | 1786.792 | 20.469 | $0.02174506 |
| Packed tables | 27/96 | 1252.125 | 21.594 | $0.01550296 |
| Schema + query | 91/96 | 916.490 | 51.562 | $0.01294963 |

| Model | JSON / 48 | Packed / 48 | Query / 48 |
|---|---:|---:|---:|
| llama-3.2-3b | 16/48 | 16/48 | 46/48 |
| mistral-small-3-2-24b-instruct | 11/48 | 11/48 | 45/48 |

| Schema family | JSON / 32 | Packed / 32 | Query / 32 |
|---|---:|---:|---:|
| cargo | 5/32 | 5/32 | 29/32 |
| teams | 12/32 | 11/32 | 32/32 |
| paths | 10/32 | 11/32 | 30/32 |

Query correctness: 83/96 on the first attempt, 91/96 after at most one correction, and 91/96 final answers. Retries: 9. Correct execution followed by a wrong final answer: 0. Valid but semantically wrong SQL does not trigger correction.

## Token check and spending

Incomplete-exchange spend included in the accuracy total: $0.000077700000. The resumed run retains its full original call ledger; its $0.12 cap is cumulative, not an additional allowance.

One-token replies were measured first on one fixed question per state/format/model (36 calls). These measure initial prompt size, not completed-exchange cost or accuracy.

| Model | JSON input | Packed input | Query input |
|---|---:|---:|---:|
| llama-3.2-3b | 1774.167 | 1243.833 | 390.000 |
| mistral-small-3-2-24b-instruct | 1797.333 | 1258.333 | 388.000 |

Token check: $0.005019562500 (cap $0.01). Accuracy run: $0.050275356250 (cap $0.12). Total: **$0.055294918750**. Cost uses reported prompt/completion usage and the Venice price snapshot saved with each run; provider cache discounts are not assumed. One network attempt per call, no automatic transport retries. Unknown/failed calls retain their worst-case reservation and stop the run.

## Remaining errors

Five completed query exchanges were wrong: Llama added an unrequested `LIMIT 1` to two cargo list queries; Mistral repeated unsupported subqueries for one cargo top-three total, and added an unrequested `access = 'open'` filter to two route top-three totals. These are recorded failures, not corrected scores. The native engine cannot infer that valid SQL has the wrong intent.

Packed and full JSON have the same aggregate score, but not identical per-case outcomes: five cases were correct only with JSON and five only with packed tables.

## Frozen method and scope

- Cargo seeds 11101/11102: 24/48 rows; teams 11201/11202: 16/32 rows; routes 11301/11302: 20/40 rows. Eight questions per state, 48 per model/format, 288 scored exchanges total. All states and prompts were frozen before the token check. An adapter bug around a retriable session EOF interrupted the run after 15 completed cases; those answers were reused unchanged. One incomplete exchange was restarted, with its cost retained. No tuning or repeats selected by score.
- JSON and packed formats receive identical field meanings and all rows. Query mode receives native schema/meanings with no numeric rows, executes Rust queries locally, and includes all tool exchanges in its usage totals. Table packing is lossless; format order rotates by question number.
- Questions cover categorical/numeric filters, counts, totals, averages, extrema and top-three sums. Independent Python comprehensions provide expected answers; all 48 oracle SQL results are checked against both Rust and Python before any paid request.
- Strict JSON replies, optionally enclosed in a code fence. Duplicate keys, nonfinite values and trailing prose are rejected. Numeric tolerance is relative 1e-6; ID lists are unordered but duplicate-sensitive. Boolean values are not numeric answers.
- Temperature zero, maximum 320 completion tokens per accuracy call, only Llama 3.2 3B and Mistral Small 3.2 24B. Hosted inference can still vary; there is one completed exchange per case, no significance or broad generalization claim.
- Three schema families broaden the earlier item-only semantic test, but all are small synthetic tabular tasks with related operations. They do not measure live gameplay, long sessions, model-authored brain quality, cited-hypothesis truth or causal effects.

Released code commit: `5fe26917c08d3468b7a8764ba7e99188aaa9b8e0`. Evaluation script SHA-256: `ef0366b23d67011060da5785c4f6b283cd1274562ec9c5983e184ad8b741422b`.

## Artifacts and reproduction

[Accuracy answers, queries, frozen inputs and usage](../../eval/query/release-model-results.json) · [Initial-token run](../../eval/query/release-model-tokens.json) · [Interrupted run retained for audit](../../eval/query/release-model-interrupted.json) · [Runner](../../eval/query/release_model_eval.py) · [Offline report generator](../../eval/query/summarize_release_model.py).

Offline checks (no key or API calls):

```bash
python3 -I eval/query/test_release_model.py
python3 -I eval/query/release_model_eval.py --check --cap 0 --output eval/query/release-model-tokens.json
python3 -I eval/query/release_model_eval.py --check --cap 0 --output eval/query/release-model-results.json
python3 -I eval/query/summarize_release_model.py --check
```

Paid reruns require new output paths; do not overwrite the frozen artifacts or run them in CI:

```bash
mkdir -p eval/results
python3 -I eval/query/release_model_eval.py --tokens-only --cap 0.01 --output eval/results/release-tokens-new.json
python3 -I eval/query/release_model_eval.py --cap 0.12 --output eval/results/release-accuracy-new.json
```
