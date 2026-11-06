# gamectx formats

Everything gamectx reads and writes is plain JSON, so a game in any language can produce
the inputs or read the packed output without this crate. A port is conformant when it
passes the shared cases in [conformance/](../conformance/README.md).

## Compact serialization

Packed output is compared byte for byte, so every implementation must serialize the same way:

- No whitespace outside strings. Object keys keep their order: input order for parsed
  objects, and the documented order for packed shapes.
- Numbers are written exactly as their source text (`1.50`, `1e3`, `-0` and integers
  beyond 2^53 are preserved). Numbers that gamectx creates itself, such as counts, offsets
  and byte sizes, are plain base-10 integers; numbers changed by a `round` view are plain
  decimals without trailing zeros.
- Strings escape only `"` → `\"`, `\` → `\\`, the short forms `\b \f \n \r \t`, and other
  characters below U+0020 as lowercase `\u00XX`. Everything else, including `/`, `<`, `>`,
  `&`, `=`, `'` and all non-ASCII characters, is written as raw UTF-8.

> **Java/Gson ports:** Gson escapes `<`, `>`, `&`, `=` and `'` as `<` etc. by default.
> Use `new GsonBuilder().disableHtmlEscaping()`, and never route numbers through `double`
> (`getAsDouble()`, or Gson's default mapping to `Double`), which turns `1.50` into `1.5`.
> The `parse/escapes` and `table/exact-numbers` cases catch both.

Parsing is strict: duplicate keys, leading zeros, raw control characters in strings,
unpaired surrogates, trailing text and nesting deeper than 64 are errors.

## Manifest

| Field | Type | Meaning |
|---|---|---|
| `budget_bytes` | integer, optional | Maximum bytes of compact JSON for the whole packet. |
| `formulas` | array, optional | Formula definitions (`{"name","params","body","description"}`) that table `derive` expressions may call. See [Formulas](#formulas). |
| `budget_tokens` | integer, optional | Maximum estimated input tokens (see [Calibration](#calibration)). If both budgets are set, the packet must fit both. |
| `sections` | array | The sections, in declared order. |

Each section:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | Key in the packet; must be unique. |
| `pack` | string | `raw` | `raw`, `table`, `signatures`, `schema`, `outline`, `catalog` or `memory`. |
| `value` / `file` | any / path | one required | Inline data, or a JSON file relative to the manifest. |
| `priority` | integer | 0 | Higher is more important. Lower priorities are dropped first; ties drop the later-declared section first. |
| `stable` | bool | false | Rarely changes between requests. Stable sections are placed first, keeping the start of the packet identical so it can be cached. |
| `required` | bool | false | Never dropped. Assembly fails if required sections alone exceed the budget. |
| `group_by` | string or string array | none | `table` only: group rows under this column's values, or nest groups by several columns, outermost first. Grouping columns must hold strings. |
| `derive` | object | none | `table` only, view: new columns computed by [formula](#formulas) expressions, e.g. `{"danger": "danger(hp, dist)"}`. |
| `where` | object | none | `table` only, [view](#views): keep rows meeting every condition, e.g. `{"kind": "weapon", "count": {">=": 15}}`. |
| `relevant_to` | string | none | `table`: keep the rows that best match the words of this text ([view](#views)). `catalog`: return the entries that best match it, instead of a `query` search. |
| `count` | integer | 8 | `catalog` and `memory`: how many entries or records to return. |
| `relevant_fallback` | string | `all` | `table` with `relevant_to`: `none` sends no rows when nothing matches, instead of every row. |
| `sort_by` | string | none | `table` only, [view](#views): sort rows by a column; `"-col"` sorts descending. |
| `limit` | integer | none | `table` only, view: keep the first N rows after sorting. |
| `fields` | string array | all | `table` only, view: keep only these columns, in this order. |
| `round` | object | none | `table` only, view: `{"column": places}` rounds numbers to that many decimals. |
| `query`, `kind`, `offset`, `count` | | `""`, any, 0, 8 | `catalog` only: the search to run. |

Unknown keys are errors, so a typo never silently does nothing.

## Packed shapes

### `table` (lossless)

Input: an array of objects that all have the same keys.

```json
{"columns":["id","kind","count"],
 "rows":[["salt_pouch","reagent",12],["ember_lantern","tool",1],["mend_moss","reagent",7]]}
```

With `group_by`, the column moves out of the rows and becomes the group keys. Groups appear
in the order their value first appears; rows keep their order within a group.

```json
{"group_by":"kind",
 "columns":["id","count"],
 "groups":{"reagent":[["salt_pouch",12],["mend_moss",7]],"tool":[["ember_lantern",1]]}}
```

With several columns, groups nest, outermost first; a list of one column is written as a
plain string:

```json
{"group_by":["kind","rarity"],
 "columns":["id","count"],
 "groups":{"weapon":{"unique":[["ash_knife",3]],"common":[["salt_sling",9],["rust_sling",4]]},
           "food":{"rare":[["moss_cake",2]],"common":[["moon_trout",8]]}}}
```

(The expected output of conformance case `table/grouped-nested`.) An empty table packs to
empty `columns` and `groups`.

Unpacking a grouped table restores every row and value, with rows in group order and the
group columns first in each object. Values are never replaced with lookup indexes; see
the measurements in the README for why.

### Views

A table section can shape its rows before packing. Steps always run in this order:

0. `derive`: add each new column, computed by a formula expression whose names are the
   row's columns. A row whose used inputs are missing or not numbers, or whose evaluation
   fails (such as dividing by zero), gets `null`. Adding a column that exists is an error.
1. `where`: keep rows meeting every condition. A plain value means equal (`1` equals `1.0`);
   otherwise one operator object: `{"=": v}`, `{"!=": v}`, `{">": n}`, `{">=": n}`, `{"<": n}`,
   `{"<=": n}` or `{"in": [v, ...]}`. Ordering operators compare numbers only; a cell that
   is not a number (such as `null`) does not match them. A missing column is an error.
2. `relevant_to`: split the text into words (runs of letters, digits and `_`, lowercased),
   ignoring all-digit words and also trying each word of four or more letters without a
   trailing `s`. A word matches a row when it equals one of the row's string cells, or one
   `_`-separated part of one, lowercased. Rows with the most matching words are kept; if no
   row matches any word, every row is kept, or none with `"relevant_fallback": "none"`.
3. `sort_by`: stable sort by one column. Numbers compare as numbers, strings by Unicode
   code point; a column that mixes the two is an error. Nulls go last in both directions.
4. `limit`: keep the first N rows.
5. `fields`: keep only the listed columns, in the listed order. Because filtering and
   sorting come first, they can use a column you do not send, such as distance to the
   player.
6. `round`: round numbers in a column to N decimals (0–12), trimming trailing zeros
   (`3.50` → `3.5`, `-0.04` at 1 place → `0`). Numbers without a fraction or exponent are
   left exactly as written. Nulls are left alone; other values are an error.

A view drops information on purpose, so `verify` reports such sections as "round-trips
the rows its view selected": the packed table restores exactly the viewed rows.

### `signatures` (lossless)

The recommended way to describe actions in a prompt: tool schemas as short text lines,
packed into the packet as one JSON string. Input: the same tool array as `schema`.

```text
All arguments required; no others allowed.
Shared arguments:
  target: string; Entity handle.
  power: integer 1..5
cast(spell, target, power): Cast.
  spell: one of spark|hush
guard(target, power): Guard.
wait(seconds): Wait.
  seconds: integer 1..30
```

(The expected output of conformance case `signatures/shared`.)

- The first line is always the header shown. Each action is `name(arg, arg): description`,
  followed by two-space-indented lines for its arguments.
- An argument whose name and rule are identical in two or more actions, and whose name
  means nothing else, is described once under the "Shared arguments" line instead.
- Argument rules: `one of a|b|c` (string enum); `string`, `integer`, `number` or `boolean`;
  integers and numbers may add ` min..max` (either side may be empty, e.g. `1..` or `..9`);
  strings may add `, at most N characters`; any rule may end with `; description`.
- Every tool must be strict (`strict: true`, every property required, no additional
  properties) with a one-line description. Names use letters, digits, `_` and `-`. Enum
  values may not contain `|`, `;` or line breaks. Anything else, such as nested objects,
  arrays, patterns or numeric enums, is refused; use `schema` for those tools.
- Unpacking restores each tool as a bare function object (`{"name","description","strict",
  "parameters"}`); a provider's `{"type":"function"}` wrapper is not kept.

### `schema` (lossless)

Use this for action catalogs sent as context data when `signatures` cannot express them
(nested objects, arrays and other JSON Schema features). Do not send packed schemas in a
provider's native `tools` / function-calling parameter; providers require their own format
there.

Input: an array of tools, either all wrapped (`{"type":"function","function":{...}}`) or
all bare (`{"name","description","strict","parameters"}`).

The expected output of conformance case `schema/wrapped-shared-defs` (four tools; `target`
and `power` repeat three times, so they are shared):

```json
{"wrapper":"function",
 "columns":["name","description","properties","extra"],
 "extra_null":"extra null = strict:true and parameters {type:object,additionalProperties:false,required:all property keys}",
 "$defs":{"p0":{"type":"string","description":"Entity handle."},"p1":{"type":"integer","minimum":1,"maximum":5}},
 "rows":[["cast","Cast.",{"spell":{"type":"string","enum":["spark","hush"]},"target":{"$ref":"#/$defs/p0"},"power":{"$ref":"#/$defs/p1"}},null],
         ["guard","Guard.",{"target":{"$ref":"#/$defs/p0"},"power":{"$ref":"#/$defs/p1"}},null],
         ["use","Use.",{"item":{"type":"string"},"target":{"$ref":"#/$defs/p0"},"power":{"$ref":"#/$defs/p1"}},null],
         ["wait","Wait.",{"seconds":{"type":"integer","minimum":1,"maximum":30}},null]]}
```

- A parameter rule becomes a shared `$defs` entry only when it repeats and the
  references save more than the definition costs.
- `extra` is `null` for the common strict form. Otherwise it is
  `{"strict":...,"parameters":{...every parameter key except type and properties}}`.
- Source schemas whose top-level properties already use `$ref` are refused. Pack the
  expanded form instead.

### `outline` (lossy)

Input: the same tool array. Output: one string, `name(arg=a|b,arg:min..max,arg);...`.
Descriptions, types and nested rules are dropped.

### `catalog` (a search page)

Input: `{"complete":true,"entries":[{"kind":"item","id":"ember_lantern","tags":["#lights"]}]}`
or a bare array of entries. IDs, kinds and tags may not contain spaces or control
characters. Tags are sorted, and at most 8 are kept per entry.

Output of `gamectx search examples/ember-keep/catalog.json lights --kind item --count 1`:

```json
{"generation":"5ec1d79bdf3831c5","complete":true,
 "columns":["kind","id","tags"],
 "rows":[["item","ember_lantern",["#fire","#lights","#tools"]]],
 "next":1,
 "meaning":"registered metadata only; not availability, ownership or permission"}
```

- A query matches entries whose ID or tags contain every word (up to 4). A leading `#` is
  ignored when matching, and a trailing plural `s` is optional.
- `next` is the offset of the next page, or `null`. Offsets stop at 4096.
- With `relevant_to` instead of `query`, entries are ranked by how many words of the text
  match their ID, its `_`-separated parts, or their tags (without `#`; a trailing plural `s`
  is optional on both sides). The best `count` entries are returned, ties in catalog
  order, and entries matching nothing never are; `next` is `null`.
- `generation` is a 64-bit FNV-1a fingerprint of the catalog. Passing it back
  (`--generation`) makes a later page fail instead of mixing old and new data. It
  detects changes; it is not a security hash.
- A trailing `"..."` tag means the entry had more tags than were kept.

### Formulas

A safe expression language that a game, or its model through the `define_formula` and
`calc` tools, uses to describe calculations the game then performs exactly.

- Values are numbers and `true`/`false`. Parameters and row columns are numbers.
- Operators, loosest first: `or`; `and`; `not`; `< <= > >= == !=` (not chainable);
  `+ -`; `* / %`; unary `-`. Parentheses group.
- Built-ins: `min(...)`, `max(...)`, `abs`, `floor`, `ceil`, `sqrt`, `clamp(x, low, high)`,
  `round(x)` or `round(x, places)` (0–12), and `if(condition, then, else)`, which only
  evaluates the branch it takes.
- A formula may call formulas defined before it, never itself, so recursion and loops are
  impossible. Types, names and argument counts are checked at definition.
- Limits: 64 formulas, 8 parameters, 300 characters and 200 parts per body, nesting 32,
  and 10,000 evaluation steps per evaluation.
- Division or remainder by zero, `sqrt` of a negative number, and non-finite results are
  errors. Results print as integers when whole, otherwise in shortest form
  (`0.1 + 0.2` is `0.30000000000000004`; use `round`).

Saved formulas are `[{"name","params","body","description"}]` in definition order.
`gamectx calc "<expression>" --formulas FILE` evaluates one expression from the command
line.

### Queries

Queries are model-written requests executed over the game's own named tables, separate
from packet packing. The CLI takes `gamectx query TABLES.json "SELECT ..."`, where the
file is an object of table names to arrays of row objects. Rust takes a slice of
`(name, rows)` pairs through `query::run`, or parses once with `Query::parse` and runs
against updated rows with `Query::run`. `gamectx query --tool` / `query::tool()` returns
a strict function schema that packs as a signature.

Supported grammar, in order:

```text
SELECT expression [AS name] [, expression [AS name] ...]
FROM table
[WHERE column comparison literal [AND column comparison literal ...]]
[ORDER BY column [ASC | DESC]]
[LIMIT n]
[;]
```

- Names are ASCII letters, digits and `_`, starting with a letter or `_`. Keywords and
  functions ignore case; table and column names are case-sensitive. Spaces, tabs, CR and
  LF separate tokens. Quoted identifiers, comments and string escapes are unsupported.
- Expressions: numeric constants, columns, parentheses, unary `-`, binary `+ - * / %`
  with normal arithmetic precedence, `ABS(x)`, `ROUND(x)` and `ROUND(x, places)` (0–12).
  Numeric constants have digits with an optional fractional part; no exponent syntax.
- Aggregates: `COUNT(*)`, `COUNT(x)`, `SUM(x)`, `AVG(x)`, `MIN(x)`, `MAX(x)`. Their argument
  may contain arithmetic on columns. Aggregate results can be combined with arithmetic,
  e.g. `MAX(trust) - MIN(trust)`. Aggregates cannot nest, and columns outside aggregates
  cannot appear in a query containing an aggregate. Aliases are accepted and ignored;
  they cannot be used for sorting or filtering.
- Comparisons: `= != <> < <= > >=` and `IN (literal, ...)`, joined only by `AND`.
  Literals are numbers, single- or double-quoted strings, or bare words interpreted as
  strings. `null`, `true` and `false` are bare strings, not special literals. Equality
  treats numeric `1` and `1.0` as equal; inequalities require numbers, as in `where` views.
- `ORDER BY` accepts one column and sorts stably, with nulls last in either direction.
  Mixing numbers and strings, or sorting other JSON types, is an error. `LIMIT` is a
  whole number from 0 to 4,294,967,295. **Sorting and limiting happen before aggregation**:
  `SELECT SUM(weight) FROM inventory ORDER BY weight DESC LIMIT 3` sums the three
  heaviest rows. This differs from standard SQL.
- No `SELECT *`, `OR`, joins, subqueries, `GROUP BY`, `HAVING`, `UNION`, mutations or
  multiple statements. Unsupported features return errors with supported alternatives.

Each selected plain column preserves its original JSON value and number text. Arithmetic
uses IEEE 754 64-bit floats; missing numeric values, non-number cells, division/remainder
by zero and non-finite results become `null`. `ROUND` rounds halves away from zero.
Computed whole numbers with magnitude below 10^15 print as integers, others in Rust's
shortest decimal form; use `ROUND` to control floating-point tails. Aggregates skip null
numeric results; `COUNT(column)` counts every non-null value, including strings and
booleans. Empty sums and counts are `0`; empty averages, minima and maxima are `null`.

One selected expression returns an array of values; several return an array of arrays.
An aggregate query returns one scalar or, with several expressions, one array:

```json
["ember_cake","salt_pouch"]
```

```json
[["ember_cake",6],["salt_pouch",14]]
```

```json
[23,2]
```

Default limits: 500 Unicode characters, 16 selected expressions, 16 conditions, 64 values
per `IN`, 200 expression nodes, nesting depth 32. Hosts may change these with
`Query::parse_with` and `query::Limits`. Every row must be an object, and every referenced
column must exist in every row, even rows that a filter would remove; an empty table
cannot validate column names. Errors list available tables or columns where possible.
Execution work scales with the host's table size, bounded expression shape and an
optional sort; the query cannot request loops, external data, or modification. Hosts
control table sizes and access, and must budget the result before sending it to a model.

#### Table descriptions and correction sessions

`query::describe(tables)` / `gamectx query --schema TABLES.json` describes the same named
arrays accepted by queries. `--schema -` reads JSON from stdin. The description is:

```json
{"t":{"columns":[["id","string"],["kind","string"],["x","null|number"]],"values":{"id":["a","b"],"kind":["food","tool"]}}}
```

Table order and column order follow the input and the first row. Mixed types are joined
with `|` in the fixed order `null`, `boolean`, `number`, `string`, `array`, `object`.
All rows must be objects with the same columns; different key orders are accepted.
An empty table has `{"columns":[]}`. There is no row count or numeric sample.

`values` includes only columns holding strings in every row whose *complete* domain
has at most 8 distinct values, each at most 64 Unicode characters. Domains retain first
appearance order. A larger domain, a longer string, or a non-string cell omits the whole
column's domain; no partial enumeration is presented. `values` is omitted when empty.
This can include IDs in small tables. Descriptions allow at most 16 distinct table names
and 64 columns per table; names must be unique ASCII SQL identifiers of at most 64
characters. Hosts can provide their own descriptions when inferred empty schemas or these
limits do not suit their data. Field meanings come from host prompts, not type inference.

`query::Session` allows one query and one correction after an execution error. A successful
query ends the session immediately, including when its meaning is wrong for the question.
`apply_call` accepts exactly `{"sql": string}`; malformed arguments consume an attempt.
`submit` accepts SQL text directly. Both return these protocol envelopes:

```json
{"result":23}
```

```json
{"error":"Unknown table. Use a table name listed below.","retry":true,"tables":{"inventory":{"columns":[["stack_weight","number"]]}},"hint":"Use only the tables and columns below. No subqueries; one SELECT only. For the largest row use ORDER BY column DESC LIMIT 1. ORDER BY and LIMIT apply before SUM/AVG/MIN/MAX, including top-N sums."}
```

The first error includes the entire description and `query::RETRY_HINT`. Error text is
normalized: unknown table, unknown column, result budget exceeded, or invalid/unsupported
SELECT. The second failure returns only `error` and `retry:false`. Submissions after a
terminal reply return `Err`; errors describing invalid host tables also close the session.
The host owns the model/API loop and returns feedback as tool data, with the same data
snapshot for both attempts. Creating a new session for every attempted correction would
defeat the bound. A session makes no network calls.

The compact result is capped at 4096 UTF-8 bytes by default, excluding its envelope;
`.result_bytes(n)` changes that limit. Oversize results allow the same single correction,
with advice to select fewer columns or add LIMIT. The output cap does not bound table
execution work or feedback size: hosts control table sizes and budget whole messages.

`gamectx query --session TABLES.json [--result-bytes N]` reads JSON argument objects from
stdin, one per line, and immediately flushes one JSON reply per attempt. It reads at most
8192 bytes per line, accepts escaped newlines in SQL, and stops after success or the second
failure. Malformed JSON consumes an attempt. Terminal query errors are normal protocol
replies (exit success); premature EOF, oversize input lines, invalid host data and I/O errors
fail the command. A table file is required because stdin carries the requests.

### `memory` (recall)

Input: saved memory, as written by `Memory::to_value`:

```json
{"next_id":3,"records":[
  {"id":1,"turn":4,"importance":6,"text":"The player gave Ilse a salt pouch.","tags":["player","gift"]},
  {"id":2,"turn":9,"importance":8,"text":"An ash hound was seen near the well.","tags":["danger"]}]}
```

Records have a unique `id` below `next_id`, the game's `turn`, an `importance` from 1 to
10, one line of `text` (up to 280 characters) and up to 8 single-word lowercase `tags`.
The section returns the `count` records most relevant to `relevant_to`, as a table ordered
by turn:

```json
{"columns":["id","turn","text","tags"],"rows":[[2,9,"An ash hound was seen near the well.",["danger"]]]}
```

Relevance: each word of the text (the same word rules as table views) found in a record's
text or tags (a trailing plural `s` optional on both sides) adds `1/n`, where `n` is how
many records contain that word. Rare words count fully; words in nearly every record
barely count, in any language. Ties go to higher importance, then the later turn. With no
matching words, the most important, most recent records are returned.

## Reaction brains

`brain::Brain` is a small neural utility policy, independent of engine, genre and action
execution. Hosts bind normalized observations to named inputs, supply desired utility
labels for learning, select eligible outputs and own storage/scheduling. Scores are not
probabilities or facts. Native action validation stays outside the network.

### Definition and inference

```json
{"version":1,"inputs":["danger","need"],"outputs":["withdraw","assist"],"nodes":[
 {"id":"withdraw","weights":{"danger":1},"bias":0,"pinned":false,"learning_limit":0.5},
 {"id":"assist","weights":{"need":1,"danger":-1},"bias":0,"pinned":false,"learning_limit":0.5}]}
```

All fields shown are required; unknown fields are errors. Names match
`[a-z][a-z0-9_]{0,31}`. Inputs are distinct; node IDs are distinct from inputs and earlier
nodes. Every connection names an input or an earlier node: forward references, cycles
and duplicate connections are refused. Outputs are distinct existing node IDs; an
output node may also feed a later node. Empty connection maps allow constant nodes.

Limits: 16 inputs, 32 nodes, 16 connections per node and 16 outputs. Weights and biases
are finite numbers −4..4. `learning_limit` is 0..4, limiting each parameter's absolute
displacement from its authored baseline. A pin locks that node's parameters, including
bias; upstream learned values may still change its activation.

All neural math uses integer millionths (`S = 1000000`). Numeric inputs/parameters are
read as finite IEEE 754 binary64, range-checked, multiplied by S and rounded half away
from zero. Generated values serialize as plain decimals with at most six fractional
digits, no trailing zeros/exponents, and zero rather than negative zero. This is
intentional quantization; the existing lossless JSON/table rules are unchanged.

For each node, accumulate `bias*S + sum(weight*source)` in signed 64-bit integers,
divide by S with half-away rounding, then clamp to 0..S. Connections with a nonzero
weight and an unknown source make that node unknown. Zero-weight connections do not
require observations. Missing/null inputs are unknown; explicit numeric zero is observed
zero. Unexpected input names, booleans, non-numbers and values outside 0..1 are errors.

`run(inputs, allowed)` returns, in order:

```json
{"revision":0,"scores":{"withdraw":0.8,"assist":0},"choice":"withdraw","margin":0.8}
```

Scores follow output-definition order, including ineligible/unknown outputs. Eligibility
must be a distinct subset of the outputs; an empty set yields no choice. If any eligible
score is unknown, `choice` and `margin` are null. Otherwise margin is best minus runner-up
(or best minus zero for one eligible output). `run_with` requires the best score and
margin to be strictly greater than host-provided thresholds (both 0..1, default zero).
Ties and all-zero eligible scores abstain. Thresholds are utility rules, not confidence.

`Brain::dense(inputs, outputs, hidden, seed)` creates a reproducible initializer. Hidden
width 0..16 (zero means no hidden layer); generated IDs are `hidden_0`, etc., with normal
collision checks. All nodes start with bias 0.5, unpinned, learning limit 4. Connection
weights are integer ticks in −500000..500000: use unsigned 64-bit xorshift
`s ^= s << 13; s ^= s >> 7; s ^= s << 17` with wrapping shifts, then `s % 1000001 - 500000`.
Seed zero maps to one. Draw connections in source order, hidden nodes first, outputs last.
Initialization encodes no knowledge of a game.

### Lessons, training and persistence

`teach` requests contain exactly `id`, `expected_revision`, `changes`, `validation`.
Each of 1..16 changes is `{node,source,delta}`; source is an existing connection name or
`$bias`. Changes cannot repeat a parameter. Delta is nonzero after quantization and
within −0.1..0.1; pins, the absolute bounds and the baseline displacement bound all apply.
An invalid request never partially changes a brain.

`train` requests contain exactly `id`, `expected_revision`, `examples`, `validation`,
`epochs`, `rate`. Samples are `{"inputs":{...},"targets":{...}}`; every input and output
is required with a numeric 0..1 value. Training and validation each have 1..128 samples;
their normalized input vectors cannot overlap on the millionth grid. Epochs are 1..128,
rate is greater than zero and at most one after quantization. No shuffling or randomness
occurs during learning. Labels come from the caller; synthetic proposals are not evidence.

Training runs on a candidate copy, in supplied sample order for each epoch. Backpropagate
output error `score-target` through the graph in reverse node order. At hidden nodes,
the clamp derivative is one only when rounded pre-clamp activation is strictly within
0..S. Output nodes use a straight-through derivative of one, including saturation, so
an old saturated preference can be unlearned. Clip each node delta to −S..S after its
derivative. Propagate `round(delta*weight/S)` to upstream nodes; weight gradients are
`round(delta*source/S)`, bias gradient is delta. Pins suppress parameter updates but do
not block propagation through fixed connections. Calculate all gradients before changing
any parameter for that sample. Update by `parameter - round(rate*gradient/S)`, clamped
to both −4..4 and the authored baseline ± learning limit.

Preflight work is `(3*epochs*training_count + 2*validation_count) * parameter_count`,
with one parameter per bias/connection. More than 500000 estimated work units is refused
before training. Parsing/shape checks are separately bounded; this estimate is not a
CPU instruction count or wall-clock guarantee. Hosts choose when to run synchronous training.

Both lessons and training compare validation mean squared error over all samples/outputs,
rounded to millionths. Only a strictly smaller validation loss commits the candidate:

```json
{"accepted":true,"revision":1,"before_loss":0.167654,"after_loss":0.002275,"steps":33880}
```

The report's `after_loss` describes the candidate even when rejected. A rejection leaves
weights, revision, used IDs and history unchanged. Lower validation MSE is not a guarantee
of improved action accuracy or real-world outcomes. Use trusted labels and additional
host metrics for deployment decisions.

Accepted updates consume a unique 32-character name ID and increase revision. There are
at most 128 accepted IDs per authored brain. Every request must match the current
revision; stale or replayed IDs fail. Undo requires `expected_revision`, restores the
latest retained parameter snapshot and advances revision without freeing used IDs.
Eight snapshots persist; older changes remain learned but cannot be undone individually.

`to_value` / `restore` use these ordered fields: `version:1`, `definition` (authored
baseline), `revision`, `weights`, `used_ids`, `history`. Each matrix has one row per node:
bias first, then weights in that node's definition order. History entries are `{id,weights}`
with the pre-update snapshot, oldest first. Restoration validates every matrix shape,
bound, pin, displacement, unique ID and ordered history membership; revision is between
accepted-ID count and twice that count, with enough remaining undo capacity for history.
An unlearned state must equal its baseline. This is consistency checking, not authentication.
Examples and observations are not saved. `inspect` exposes current definition/revision;
`definition` returns the original authored graph.

### Selecting rates for reaction accuracy

`Brain::train_select` is opt-in; `train` keeps its existing behavior and output. Its
request has exactly `id`, `expected_revision`, `examples`, `validation`, `epochs`, `rates`,
`max_steps`. Samples, pins, displacement bounds, revisions, IDs and train/validation
separation follow the existing training rules. `rates` contains 1..8 finite values
greater than zero and at most one, distinct after millionth quantization. Epochs are 1..128.

All trials start from the same active weights, including existing learning. For each
candidate, count correct default-threshold validation choices and calculate validation
MSE. Any uniquely predicted positive output with the highest target utility is correct;
tied best targets permit any such output. All-zero targets expect abstention. Tied
predictions abstain, as in inference. This metric assumes all outputs are eligible and
zero minimum-score/margin thresholds. Hosts using other rules need separate metrics.

A candidate is eligible only if MSE is strictly below the active brain and the correct
decision count does not fall. Select the highest correct count, then the lowest MSE,
then the earliest supplied rate on an exact tie. Retain at most one candidate, one
revision and one undo snapshot; discarded trials consume no history or IDs. If no trial
qualifies, the entire active state stays unchanged. Selection uses only the supplied
validation data; the host keeps independent test/gameplay data separate.

For R rates, E epochs, N training samples, V validation samples and P parameters,
aggregate preflight work is `(3*E*N*R + 2*V*(R+1))*P`. The caller's `max_steps` is required
and must be 1..2000000; the aggregate estimate cannot exceed it. Each trial must also
satisfy the original `(3*E*N+2*V)*P <= 500000` bound. All budget checks occur before any
training. Extra validation passes account for both MSE and decision counts.

The [runnable selection workflow](../examples/brain/selection.json) returns this first result:

```json
{"accepted":true,"revision":1,"before_loss":0.167654,"after_loss":0.000385,"before_correct":1,"after_correct":2,"validation_count":2,"selected_rate":0.3,"steps":101728,"trials":[{"rate":0.03,"loss":0.038317,"correct":1},{"rate":0.1,"loss":0.002275,"correct":2},{"rate":0.3,"loss":0.000385,"correct":2}]}
```

On rejection, `selected_rate` is null and after-metrics equal the unchanged active
brain's metrics. `trials` still describes every proposed final candidate, including
ineligible ones. Report order follows the example. Inference and saved-state formats
are unchanged. A reused validation set can overfit; the gate is not a gameplay guarantee.

### Optional prediction cache

`CachedBrain::new(brain, capacity)` owns one already parsed `Brain`, with a FIFO cache
of 0–256 predictions. Zero disables caching. A key contains every input in declared
order, normalized to the existing integer millionths (missing/null remain unknown),
the eligible-output bit mask, normalized score/margin thresholds and current revision.
Input object order and eligible-output order do not change a key. There is no additional
quantization or approximation. All input/eligibility/threshold validation runs before
lookup; a hit returns an owned prediction identical to normal inference.

`brain()` borrows read-only; `brain_mut()` clears entries before lending mutable access.
This invalidates training, lessons, undo and replacement even when the replacement has
the same revision. Rejected changes also clear conservatively. `into_brain()` returns
the owned brain. Each wrapper has independent learning and entries. `clear()` removes
entries while preserving lifetime counters. `stats()` exposes `hits`, `misses`, `entries`
and `capacity`; invalid requests count as neither hits nor misses, counters saturate,
and hits do not refresh FIFO age. Cache entries never appear in saved learning.

The cache has no clock or world access. Hosts supply current observations and eligible
reactions on every call and validate actions before execution. It caches utility
predictions, not permission to act. Retaining a compiled brain avoids reparsing at each
tick; cloning a preloaded template gives independent owners the same initial learning.

### Tools and command line

`brain::tools()` returns `create_brain`, `inspect_brain`, `run_brain`, `teach_brain`,
`train_brain`, `train_select_brain`, `undo_brain`. Every tool takes exactly one string field `request`, containing
strict JSON, so it packs losslessly as a signature. Requests are capped at 65536 UTF-8
bytes. `create_call` returns a new brain; it never silently replaces an existing instance.
The host chooses ownership, allowed feature bindings and whether to install a definition.
`Brain::apply_call` handles the six existing-brain operations. Run requests contain
`inputs`, `allowed`, `min_score`, `min_margin`; inspect takes `{}`. Train/teach match the
requests above; undo takes `{expected_revision}`. Pure Rust inference needs no tool/model call.
`Brain::parse` caps graph JSON text at 32768 bytes; `restore_json` caps state at 262144 bytes.

`gamectx brain WORKFLOW.json` (or `-` for stdin) processes strict JSON up to 262144 bytes:
exactly one of `definition`, `dense`, saved `state`, plus `operations` (at most 64 entries).
Dense configuration has exactly `inputs`, `outputs`, `hidden`, `seed` (an unsigned 64-bit
integer). Each operation is `{op,args}`, where op is `inspect`, `run`, `teach`, `train`,
`train_select`, `undo`. Output is `{"results":[...],"state":...}`. An operation error fails the workflow;
no files or external state are changed. `gamectx brain --tools` prints the tool schemas.

`gamectx brain WORKFLOW.json --cache N` uses `brain::process_cached` for one workflow,
with the same capacity bounds and byte-identical results/state. Run operations use the
cache, inspection preserves it and mutating operations clear it. The ordinary command
and `brain::process` remain uncached. Separate CLI invocations do not share entries.

## Receipt

`gamectx pack --receipt` writes this to stderr:

```json
{"columns":["name","bytes","priority","stable"],
 "included":[["actions",997,10,true],["rules",142,9,true],["inventory",444,6,false],["recent_events",227,7,false],["mood",37,8,false]],
 "dropped":[["action_outline",314,1,true],["lights_nearby",376,3,false]],
 "total_bytes":1906}
```

(The Ember Keep example. `bytes` is each section's packed size; `total_bytes` also counts
the packet's own keys and punctuation.)

## Calibration

```json
{"scope":"261cd0b79ab74ea5","samples":1,"ratio_ppm":265152}
```

- `scope` is a fingerprint of the provider/model name you chose. The name itself is not stored.
- `ratio_ppm` is the worst observed input tokens per byte, in millionths.
- An estimate is `ceil(ascii_bytes * ratio * 2) + non_ascii_bytes`, capped at the byte count.
- A usage report above the estimate that was used, or above one token per byte, resets
  the calibration. A report of zero tokens is ignored.

## Native frame work

`Brain::run_into(inputs, allowed_mask, min_score, min_margin, &mut Prediction)` takes
exactly one `Option<i64>` per declared input. Values/thresholds are integer millionths,
0..=1,000,000; None is unknown. Mask bit i corresponds to output i, with no out-of-range
bits. Fixed storage handles at most 16 inputs/32 nodes/16 outputs. Invalid arguments
leave the reusable prediction unchanged. `scores`, `choice`, `margin`, `revision` expose
native results; scores and margin remain millionths and choices are output indices.
No allocation occurs on successful native calls. JSON inference has identical semantics.

`Brain::start_train`, `start_train_select` and `start_train_select_host` validate their
requests before returning a `TrainingJob`. Requests/work limits match the corresponding
synchronous operations. The live brain is untouched while a job advances. `next_steps`
is the minimum indivisible sample budget, `completed_steps`/`total_steps` expose work,
and `advance(cap)` accepts 1..=2,000,000 units, stops at a sample boundary and never
exceeds cap. Too-small budgets error before advancing. Each training sample costs 3P;
validation costs P per sample for single-rate training or 2P for selection, where P is
parameter count. Accounting matches the existing complete-run estimate exactly.

`finish(&mut brain)` consumes a finished job, compares the full authored/learned starting
state and either installs one validated winner or reports rejection. Incomplete/stale
jobs error without mutation. Equivalent starting states may accept the same plan; this
check does not authenticate ownership. Drop cancels. Jobs are in-memory work objects,
not saved-learning formats. Copies and allocations at setup/phase transitions/finish
are bounded but not included in estimated parameter work; caps are not wall-clock limits.

### Host-policy validation

`train_select_host` uses the same request/report fields as `train_select`, but each
validation row has exactly `inputs`, `targets`, `allowed`, `min_score`, `min_margin`.
Inputs/targets remain complete, with the existing 128-sample bound and train/validation
input separation. Allowed outputs/thresholds obey inference validation. Every rate is
trained from the same starting state; MSE uses all target utilities, and correct-count
compares predicted choice/abstention with the unique winning target under that row's
mask/thresholds. Target ties/empty masks/all-zero or insufficient scores/margins expect
abstention. Default `train_select` keeps its existing tied-target acceptance behavior.

A winner needs lower MSE and no correct-count regression; choose most correct, then
lowest MSE, then earliest request rate. No winner leaves learning unchanged. The CLI
workflow op is `train_select_host`; the default model tools stay unchanged. Native hosts
may call `train_select_host` or `start_train_select_host`. Labels/masks remain host data,
and metric improvements do not guarantee held-out gameplay improvements.

### Scheduler

`Scheduler::new(capacity, frame_budget)` accepts 1..=4096 registered jobs and
1..=2,000,000 estimated units per frame. Register a unique u64 ID with positive cost
<= frame_budget, positive interval <= max_age, and initial phase < max_age. Initially
dirty jobs first become eligible at absolute tick `phase`. `changed(id, urgent)` marks
new observations/policy/eligibility; urgent overrides interval/phase, never frame budget.
`poll(tick)` reserves eligible work: urgent first, overdue next, least recently served
next (never-served counts as zero), then ID. It skips work that cannot fit remaining
budget. Periodic refresh uses elapsed max_age. The host executes reservations or marks
failed jobs changed again. `remove` frees a slot; `remaining` exposes current budget.
Ticks must be monotonic; repeated polls share budget and cannot reserve a job twice in
the same tick. Deadlines may slip under overload. No threads, clocks or engine calls
are owned by the library. Notifications/freshness/cost accuracy remain host concerns.

## Acknowledged context synchronization

`sync::Publisher` and `Replica` bind to an explicit stream name (1–64 ASCII letters,
digits, underscore or hyphen). Use a new stream/receiver after a session reset. Revision
0 means no state; wire revisions are integers 1..=2,147,483,647. Publisher exhaustion
requires a new stream. State is an ordered JSON object of at most 128 sections, names
at most 256 UTF-8 bytes, serialized state at most 262,144 bytes. Messages are bounded
at 524,288 bytes and the strict parser's normal depth. Envelopes add nesting, so a
state near the parser's maximum depth can be rejected before publication.

Every message has exactly these fields in this emitted order:

```json
{"stream":"encounter_1","base":null,"revision":1,"order":["rules","mood"],"set":{"rules":"Act legally.","mood":"calm"},"remove":[]}
```

`Publisher::publish(state, acknowledged_revision)` emits a snapshot (`base:null`, all
fields in `set`, empty `remove`) unless an existing sender state is explicitly
acknowledged at its current revision. Matching ACK emits a delta, with `base` equal to
that revision, `revision=base+1`, only changed fields in `set`, deleted fields in `remove`
and the entire ordered list of current section names in `order`. Comparisons preserve
exact JSON number text. Reordering alone changes `order`; unchanged fields stay omitted.
Every successful emission advances revision, including unchanged state. Failed publication
does not advance. ACKs must originate from the receiver for that stream.

`Replica::apply_json` bounds text before parsing; `apply` accepts native JSON values.
Receivers require their configured stream and a revision strictly greater than their
current one. Deltas also require an existing snapshot, exact base match and next revision;
snapshots may jump over dropped messages. Duplicate names, unknown removals, fields both
set/removed, extra/missing fields and inconsistent order lists are rejected. Reconstruction
is validated against state bounds before atomic installation. Any error preserves state
and revision. A baseline error requires a new snapshot; send a missing ACK to request one.
`state()` returns the full reconstructed object and `revision()` is the ACK to send.

This protocol transfers host context; it is not automatic language-model memory or a
fact-authentication mechanism. Reconstruct full packets before ordinary model calls.
Stable prefixes are retained by normal packet ordering. Provider/local-server prompt-cache
behavior and token accounting must be measured in the integration.
