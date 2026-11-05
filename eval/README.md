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
accuracy. Results remain limited to one synthetic state, with one response per question;
no retry improvement or schema-only accuracy is claimed.
