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
