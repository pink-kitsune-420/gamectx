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

For chat-driven turns where a table only matters when the message mentions it,
`"relevant_fallback": "none"` (`.relevant_only(text)` in Rust) sends no rows when nothing
matches. *Measured on the two smaller models: another 59% fewer input tokens, same accuracy
except questions about a table as a whole ("who has the highest trust?"), which then see
nothing: 2/6 → 0/6.* Those questions belong in code anyway: `sort_by` + `limit: 1` answers
"highest trust" exactly, while models given every row got it right 2 times in 6.

Catalogs take `relevant_to` too: send the entries that best match the player's message
instead of a keyword search.

Round numbers to the precision decisions need. *Measured: rounding two number columns
from two decimals to one saved 5% of the whole packet; each dropped digit costs about one
token per number. Rounding to whole numbers saved 13%, but made a 0–1 trust score useless,
so choose per column.*

Long histories are the same problem over time. Keep events in a `Memory` and send the few
it recalls for this turn instead of the whole log; let the model add to it with the
`remember` tool. *Measured: 10 questions about a 60-event history, 8 recalled events vs
all 60: 84% fewer input tokens, 20/20 correct both ways on the two smaller models.*

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
  cases one line: `{"count": {">=": 15}}`, or `.filter("count", ">=", 15)` in Rust, and
  `derive` adds computed columns from [formulas](FORMAT.md#formulas) that filters and sorts
  can use.
- **Never let the model do arithmetic itself.** *Measured: asked 8 sum, average and
  difference questions over the game data, the two smaller models got 0 of 16 right.
  Replying with an expression that gamectx computed (`calc`) got 8 of 16; every remaining
  miss picked the wrong rows, which `where` and `derive` fix in code.* Give the model the
  `calc` and `define_formula` tools (`Formulas::tools`) for arithmetic it decides on itself.
- **Otherwise:** ask for a brief list of candidate rows before the final JSON. *Measured on
  18 filtering questions: 37/54 → 46/54 correct, numeric filters 1/9 → 6/9, for about 60
  extra output tokens per answer.*
- **When the model chooses the filter:** give it `query::tool()` and execute the `sql`
  argument with `query::run()`. *Measured on 29 questions per small model: 33/58 direct,
  53/58 with SQL; numeric filters 0/6 → 6/6, whole-table questions 2/6 → 6/6, arithmetic
  1/16 → 12/16. A separate run with the tool packed as a signature scored 52/58.*
  Those historical runs still sent the full tables. Only expose tables the model may
  read, send errors back for correction, and budget the result.
- Sorting rows or adding row counts did not measurably help on its own (37 → 37 and 39
  of 54), so gamectx does not add counts and does not claim `sort_by` improves accuracy.

For a query workflow, send `query::describe(&tables)` in a per-turn variable instead of
the rows. It includes column types and complete small categorical domains. Define meanings
explicitly, such as `unit_weight` and host-derived `stack_weight = count * unit_weight`.
Use `query::Session` for one correction after an invalid request; valid SQL with the wrong
meaning ends the session, so retrying cannot repair ambiguous field semantics.

*Measured with frozen prompts on three held-out states (24/48/96 inventory rows and
10/20/40 NPCs), 29 questions per state and small model: full tables 160/174 first attempt,
171/174 after correction; schemas 169/174 first, 172/174 after correction. Returning the
result to the model preserved those scores. Across the complete exchanges, average input
fell from 4,898.632 to 1,615.874 tokens (67.0% less); completed-exchange cost fell from
$0.107952 to $0.038804 (64.1% less).* The old one-state/full-data numbers above remain
historical. These newer tests still share one synthetic game family. Mistral sometimes
persists with unsupported top-N subqueries or interprets rarity `unique` as count = 1.

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

The original packing benchmarks use one state and three models; query workflows add
three held-out states on the two smaller models. Your data and models will differ.

- `gamectx calibrate` learns your provider's real tokens-per-byte ratio from reported usage.
- The eval script runs your own questions against any OpenAI-compatible API with a hard
  spending cap. Change `build_data` and `build_questions` to match your game.

## 9. Learn small reaction preferences locally

`brain::Brain` is an engine-independent utility network. Choose meaningful normalized
inputs and reaction names in your host, then bind them to real observations and currently
legal reactions. It can serve an NPC, player-specific preference, faction, encounter
director or other owner selected by the game. Use its compact prediction as a `Turn::var`
when the model needs it; routine local inference needs no model request.

The host supplies labeled examples and separate validation states. Training happens on
a candidate copy, with work limits, pinned parameters and bounded displacement from the
authored baseline. Only lower supplied-label validation MSE is retained; rejection is
atomic. Save learning with `to_value`, restore it with validation, and undo recent updates
without freeing replayed IDs. Schedule training explicitly rather than training on every tick.
Missing inputs remain unknown, and ties/unknown eligible scores abstain for a host fallback.
The normal game executor remains responsible for legal actions and changed-world checks.

*Offline measurement on four synthetic utility tasks, 32 training and 32 validation
examples each, fresh test seeds 707/808/909: random initialization 545/1,536 correct
reactions → trained 1,421/1,536. The nonlinear switch task scored 306/384, so the result
is not uniformly perfect. Changed trading preferences improved from 264/384 to 384/384
on fresh seeds 1001/1002/1003. Saved trained states were 1,374–1,762 bytes. No API calls.*

These measure agreement with reference labels, not human enjoyment, causal reward learning
or performance against an LLM. A lower validation loss can still miss important reaction
boundaries. Keep independent gameplay tests and host metrics, and use formulas/queries
for exact calculations. Host game logic supplies feature normalization, freshness, label
meaning, permissions, storage and rollout choices. The network does not create factual
memory from proposed lessons or from its own predictions.

When reaction choice matters, use `train_select` to compare a short list of learning
rates. It selects by correct validation choices, then MSE, and retains one candidate
only if MSE improves and choices do not regress. Supply an explicit aggregate work
budget, at most 2,000,000 units; individual trials retain the 500,000-unit bound.

*Fresh paired test seeds 2001/2002/2003: a fixed rate scored 1,436/1,536, selected rates
1,487/1,536. Nonlinear switch improved 315/384 → 359/384. Both used 32 training samples
and 128 validation samples per task; rates 0.03/0.1/0.3 were selected only on validation
data. Selection used about 3× estimated training work; runtime inference is unchanged.*
This is a different cohort from the original 1,421/1,536 benchmark above. Its utility
targets and initializer are still synthetic; host eligibility masks and thresholds
need their own evaluation, and repeatedly tuning against validation data can overfit.
