# Getting more from less

Practical rules for game context, in order of impact. Each says whether it was measured
with the [eval script](../eval/model_eval.py) (three models, Llama 3.2 3B to Llama 3.3 70B)
or is standard practice that was not.

## 1. Send only the rows that matter

The cheapest token is one you never send, and a model cannot confuse a row it never sees.
Let the game choose: the 10 nearest NPCs, the 5 most valuable items, catalog search
results instead of a whole registry.

```json
{"name": "nearby_npcs", "pack": "table", "file": "npcs.json",
 "sort_by": "distance", "limit": 10, "fields": ["name", "role", "mood", "trust"], "round": {"trust": 1}}
```

`sort_by` runs before `fields`, so you can sort by a column you do not send. *Standard
practice; the savings scale with how much you leave out.*

When the game does not know exactly which rows matter, `relevant_to` keeps the rows that
best match the words of some text: the player's message, the current event, or the
question. Rows matching more words win, so "any rare weapons?" keeps rows that are rare
*and* weapons; if nothing matches, the whole table is sent. *Measured on all 53 eval
questions with the question as the text: −26% input tokens, identical accuracy on every
question type, and the answer stayed in the table every time.*

Round numbers to the precision decisions need. *Measured: rounding two number columns
from two decimals to one saved 5% of the whole packet; each dropped digit costs about one
token per number. Rounding to whole numbers saved 13%, but made a 0–1 trust score useless,
so choose per column.*

## 2. Use tables, grouped by what the game filters on

Header-plus-rows tables cut input tokens by about a third against compact JSON at the same
accuracy. Grouping by the column most questions filter on (`group_by`) saved more and
answered better. *Measured: −33% tokens for tables, −38% grouped; grouped 140/159 correct
vs 133/159 for JSON; filters on the grouped column 18/18 vs 14/18.*

If questions filter on two columns, group by both: `group_by: ["kind", "rarity"]` nests
rarity groups inside kind groups. *Measured on the two smaller models: "every rare weapon"
questions 7/12 → 12/12, rarity lookups 7/8 → 8/8, 2–3% fewer tokens.*

Writing tables as plain comma-separated text instead of JSON arrays saved nothing:
tokenizers already treat `","` as one token. *Measured: 0 to +1% tokens.*

## 3. Describe actions as signatures

Action schemas are often the largest part of a packet. `signatures` writes them as short
lines (`power: integer 1..5; Effort level`) and describes arguments shared between actions
once. Its two header lines are deliberately short; longer explanations measured no better. Use `schema` only for tools that need nested objects or arrays. *Measured: 12–14%
fewer tokens than packed JSON Schema on all three models; 141/159 correct overall vs 140
for grouped tables with JSON Schema; actions 30/30.*

## 4. Never replace values with lookup indexes

Replacing repeated strings with indexes into a symbol list saved no tokens, because common
words are already one token, and models answered with the index instead of the value.
gamectx does not do this. *Measured: 102 → 74 of 108 correct.*

## 5. Do filtering in code, or let the model think first

Asking a model to filter long lists is unreliable in every format. "Items with count at
least 15" was right 1 time in 27 without help.

- **Best:** compute the answer in game code and send the result. `where` makes the common
  cases one line: `{"count": {">=": 15}}`, or `.filter("count", ">=", 15)` in Rust.
- **Otherwise:** ask for a brief list of candidate rows before the final JSON. *Measured on
  18 filtering questions: 37/54 → 46/54 correct, numeric filters 1/9 → 6/9, for about 60
  extra output tokens per answer.*
- Sorting rows or adding row counts did not measurably help on its own (37 → 37 and 39
  of 54), so gamectx does not add counts and does not claim `sort_by` improves accuracy.

## 6. Keep stable sections identical every turn

Providers that cache prompt prefixes only reuse bytes that match exactly. gamectx
serializes deterministically and puts stable sections first; keep timestamps, turn
counters and anything else that changes out of them. If a budget may drop a stable
section, the cached prefix changes that turn, so give stable sections high priority or
mark them `required`. *Observed: the 70B model reported cached input tokens on 306
of its 324 requests across all runs.*

## 7. Let the game validate every action

Packed context tells the model what exists; it does not make the model obey it. Check
every proposed action in code, and use your provider's structured output (JSON schema)
mode where available so replies are always parseable. *Measured: action choice was 29–30
of 30 in every format, but production games should not rely on that.*

Packed `schema` sections are for actions described in the prompt. When you use a provider's
native tool calling, pass the tools in its own format; gamectx can still pack everything else.

## 8. Measure your own game

These numbers come from one game state and three models. Your data and models will
differ.

- `gamectx calibrate` learns your provider's real tokens-per-byte ratio from reported usage.
- The eval script runs your own questions against any OpenAI-compatible API with a hard
  spending cap. Change `build_data` and `build_questions` to match your game.
