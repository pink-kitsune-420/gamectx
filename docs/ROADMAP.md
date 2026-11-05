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

## Measured and rejected

| Idea | Why not |
|---|---|
| Symbol/lookup indexes | no token saving; accuracy 102 → 74 of 108 |
| Plain-text (CSV-like) tables | 0 to +1% tokens: `","` is already one token |
| Row counts in table headers | +2/54 (noise) for +3% tokens |
| Sorting rows to help filtering | no accuracy change on its own |
| All-text packet instead of JSON | only ~1% better than signatures inside JSON |
| JSON query interface alongside SQL | 52/58 vs SQL 53/58 (noise), but more input tokens on both models: 1,601/1,616 vs 1,569/1,586; invented `group_by` filters. Keep SQL as the one shipped query interface. |

## Next candidates

- Catalog `relevant_to` measured with model questions (only checked by conformance cases so far).
- Query error correction with one retry, especially invented table names and unsupported subqueries; no measured retry gain yet.
- Query tools with table schemas instead of full rows; measure accuracy and the complete result-return round trip before claiming token savings.
- Sending only what changed between turns, for games that keep chat history.
- Provider prompt-cache discounts; the small-model query token check omitted cached-token accounting, so discounts remain unverified.
- Ports (TypeScript first, then C#/Python) checked against `conformance/`.
- Feedback from the first real game integration once its gaps list arrives.

## Known gaps

- Word matching is literal plus an optional plural `s`: "lighting" does not match "light".
- Numeric filtering by reading rows fails in every format; use `where` or have the model request a `query`.
- Query is a SQL subset: one table, AND/IN, no joins, subqueries or GROUP BY; ORDER BY/LIMIT run before aggregates. Arithmetic uses 64-bit floats.
- Eval data is one synthetic game state; real games may differ.
