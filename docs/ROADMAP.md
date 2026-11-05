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

## Measured and rejected

| Idea | Why not |
|---|---|
| Symbol/lookup indexes | no token saving; accuracy 102 → 74 of 108 |
| Plain-text (CSV-like) tables | 0 to +1% tokens: `","` is already one token |
| Row counts in table headers | +2/54 (noise) for +3% tokens |
| Sorting rows to help filtering | no accuracy change on its own |
| All-text packet instead of JSON | only ~1% better than signatures inside JSON |

## Next candidates

- Catalog `relevant_to` measured with model questions (only checked by conformance cases so far).
- Combine `calc` with game-side selection (`where`/`derive`) in an eval to confirm the 8/16 misses disappear.
- Sending only what changed between turns, for games that keep chat history.
- Provider prompt-cache measurements where cached tokens are discounted.
- Ports (TypeScript first, then C#/Python) checked against `conformance/`.
- Feedback from the first real game integration once its gaps list arrives.

## Known gaps

- Word matching is literal plus an optional plural `s`: "lighting" does not match "light".
- Numeric filtering by the model fails in every format; use `where`.
- Eval data is one synthetic game state; real games may differ.
