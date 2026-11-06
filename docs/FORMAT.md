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

## Versioned feature bindings

`FeatureSchema` parses strict JSON up to 16,384 bytes, with exactly `version:1`, `id`,
`revision` (1..1,000,000) and 1..16 ordered `features`. ID/feature names follow brain
names. Every feature has exactly `name`, `unit` (1..32 non-control characters), `min`,
`max`, `invert` (boolean), `missing` (`unknown`/`error`) and `outside`
(`unknown`/`error`/`clamp`). Physical bounds/observations must be finite numbers within
±1,000,000,000 units. Bounds and observations first round to physical millionths;
min must remain strictly below max. Normalization rounds `(value-min)/(max-min)` to
millionths with i128 intermediates, then optionally subtracts from 1,000,000.
Missing/null and out-of-range policies are independent. No implicit zero/unit conversion.

`normalize` accepts named raw values in any object order, rejecting unknown/duplicate
names. `Features::as_slice` returns declared-order fixed storage. `validate(brain)`
requires the identical ordered input names. `BoundBrain::new` owns both; `run_into`
validates compatibility again after mutable access, normalizes, then uses native inference.
Errors leave its caller's prediction unchanged. Binding cannot determine whether a host
actually measured the units it claims.

Bound saved state is `{"version":1,"schema":SCHEMA,"brain":SAVED_BRAIN}`. Restore takes
an explicit expected schema and compares the complete effective definition, including ID,
revision, order, units, ranges, inversion and policies. Equivalent numeric spellings on
the same millionth grid match. Text parsing is capped at 278,528 bytes. Old saved-brain
formats remain unchanged; binding envelopes are optional.

## Query meanings

`query::describe_with(tables, metadata)` returns ordinary descriptions with a `meaning`
field on annotated tables. Metadata is an object keyed by existing tables, at most
65,536 serialized bytes. Each table value has exactly `description` (1..280 non-control
characters) and `columns` (existing columns only, max 64). Each column has exactly:

```json
{"description":"Rarity label, independent of quantity.","unit":null,"kind":"categorical","values":{"unique":"A rarity label, not a count of one."}}
```

Units are null or 1..32 non-control characters. Kinds are `categorical` (string/null),
`quantity` or `continuous` (number/null), and `identifier` (string/number/null). Literal
meanings are an object of at most 16 currently present string values, each 1..64 characters,
with descriptions 1..280 characters; nonempty maps require a categorical column.
Duplicate, unknown or mismatched metadata is rejected. All-null columns can be annotated
with a compatible declared kind. Empty tables have no inferred columns to annotate.
Literal meanings are annotations, not an exhaustive replacement for inferred domains.
Meaning object order follows the supplied metadata. `describe` and SQL semantics are
unchanged. The CLI accepts `query --schema TABLES --metadata MEANINGS`.

## Diagnostics and replay

`Brain::diagnose` accepts the same native inputs/mask/thresholds as `run_into` and adds
`reason` to the existing JSON prediction. Reason precedence is `no_eligible`,
`unknown_eligible`, `all_zero`, `tie`, `score_threshold`, `margin_threshold`, `chosen`.
Invalid arguments error rather than becoming abstention. Reasons describe numeric
selection conditions, not causal intent or calibrated confidence.

`replay::Recorder` retains an initial saved brain and up to 256 explicit events. A trace
has exactly `version:1`, `initial`, `events`, at most 1,048,576 JSON bytes. Initial/state
snapshots obey the 262,144-byte brain bound. Events use monotonic unsigned ticks <=
4,294,967,295. A run event has exactly these fields:

```json
{"op":"run","tick":1,"inputs":[800000,200000],"mask":3,"low":0,"gap":0,"expected":{"choice":0},"elapsed_ns":null}
```

Inputs are native millionth integers/null; mask and thresholds follow inference bounds.
`expected:null` means unlabeled; `{choice:null}` labels abstention; `{choice:i}` labels an
existing output index. `elapsed_ns` is null or a host-recorded integer 0..1,000,000,000,000.
A state event is exactly `{op:"state",tick,state:SAVED_BRAIN}`; replacing graphs is explicit
and resets consecutive-choice switch accounting. Recorder errors preserve the prior trace;
full recorders stop instead of losing the initial baseline. Traces store normalized data;
keep feature schemas separately if reconstructing physical bindings is necessary.

`replay::run`/`run_json` and `gamectx replay FILE` return `reports`, `summary`, `state`.
Summary fields are `events`, `runs`, `labeled`, `correct`, `abstentions`, `switches`,
`latency_samples`, `p50_ns`, `p95_ns`, `max_ns`. Switches count differing consecutive
non-abstaining run choices; an abstention breaks that sequence. Latency percentiles use
nearest rank over supplied elapsed samples and are null when absent. They describe the
recorded run, not replay speed. Replay does not execute game actions or establish
counterfactual gameplay outcomes. `-` reads bounded stdin.

## Optional reaction stability

`StableBrain::new(brain, switch_margin, min_hold_ticks)` owns one brain with margin
0..1,000,000 and hold 0..1,000,000 ticks. `run_into(tick, inputs, SelectionPolicy, out)`
requires monotonic u64 time and first performs normal inference. Policy contains `allowed`,
`min_score`, `min_margin`, `urgent`. Unknowns, ties and other normal abstention clear the
previous choice immediately. Urgent valid predictions bypass holding. Otherwise an old
choice must still be eligible with score strictly above min_score; an invalid old choice
is dropped immediately. A different valid winner is held while the hold period remains,
then while its score gain over the previous choice is <= switch_margin. This can select
a lower-utility eligible reaction; the host measures the tradeoff.

`Selection` exposes raw `prediction()`, selected `choice()`, `held()` and `reason()`:
`raw`, `abstained`, `urgent`, `ineligible`, `held_time`, `held_margin`. Raw margin/choice
remain properties of the original inference, not of the held choice. Actual choice
changes reset the hold clock. Invalid calls leave selection/state unchanged. Mutable
brain access and explicit reset clear the previous choice while preserving monotonic
clock checks. Holding state is ephemeral and absent from saved learning.

## Scoped memory journal

`memory::Journal` is optional; existing `Memory`/`Record` formats remain unchanged.
Capacity is 1..1024 records. Native record requests have exactly `turn`, `entity`, `key`,
`text`, `tags`, `importance`, `source`, `expires`. Turn is monotonic <=4,294,967,295;
entity/key are 1..64 non-control characters. Text, tags and importance reuse legacy
normalization (one-line text <=280 chars, max eight tags <=32 chars, importance 1..10).
Source is `observed` or `proposed`; expiry is null or a strictly later bounded turn.

New records permanently supersede previous active records of the same entity/key/source.
Observed/proposed channels remain distinct. Recall first excludes superseded/expired
records, then includes proposals only when explicitly requested and no active observation
exists for that entity/key. Older superseded records do not revive after replacement
expiry, eviction or forgetting. Ranking uses the existing inverse-document-frequency
memory scorer over eligible records only. Recall rejects backward time, queries over
512 characters or counts over 64.

Eviction prefers expired/superseded records, then proposals, then lower importance,
older turn and lower ID. An inserted low-priority record may immediately be evicted;
returned IDs are not a promise of retention. `forget(id)` removes explicitly.
`apply_proposal(turn,args)` accepts exactly entity/key/text/tags/importance/ttl and forces
source=proposed with a 1..1,000,000-turn TTL. Model JSON cannot override provenance/time.
Saved journal has version/capacity/clock/next_id/records, with record fields
id/turn/entity/key/text/tags/importance/source/expires/superseded. Restore validates ID/time
order, bounds and active-channel uniqueness; text loads cap at 4,194,304 bytes.

## Evidence and hypotheses

`EvidenceMemory::new(subject, clock_id, capacity, max_hypotheses)` scopes one store to an
explicit subject (1..64 non-control chars) and host clock name (brain-style name, max 32).
Evidence capacity is 1..128; hypotheses 1..16. All operations use host-supplied bounded
logical time; no OS clock, game polling or network is owned by the library.

`observe(native_token, record, privacy)` is a host operation, accepting only journal
source=observed. Tokens must strictly increase, 1..4,294,967,295; replay/old tokens error.
Observation time must not precede the store's last mutating time. Privacy has at most eight
distinct brain-style labels. Eviction deletes metadata and dependent hypotheses. Source
records retain their original clocks/expiry; model proposals cannot renew observations.

`snapshot(now,count,max_bytes,denied)` returns a sealed `EvidenceSnapshot` plus its
`to_value` payload. Count <=16, bytes 1..16384. Privacy is filtered before priority/recency
selection. Contradicted, superseded and expired evidence is unavailable. Payload fields:
subject/clock/revision/now/evidence/hypotheses/citable. Each evidence row contains a journal
record, native token and privacy. Citable entries contain exactly id/token pairs from
retained evidence. Existing hypotheses are included only while unexpired and all dependencies
are retained. Truncation drops evidence, dependent hypotheses and references together.
An empty payload that cannot fit errors. Reads never renew time, confidence or state.

The host retains the sealed snapshot object while the model receives its payload unchanged.
If outer packing drops or rewrites the evidence, create a matching snapshot before applying replies. Submit
model replies through `propose(&snapshot, now, request, current_denied)`; the store rechecks
instance identity, revision, snapshot time, supplied IDs, current availability and privacy.
Restoring or creating another store invalidates old sealed snapshots even if state matches.
Proposals have exactly:

```json
{"id":"seeking_shelter","statement":"The actor may be seeking shelter.","confidence":0.6,"supports":[1],"contradicts":[2],"ttl":100}
```

IDs use brain-style names, statements 1..280 non-control chars. Confidence rounds to
millionths in 0..0.95; zero retracts an existing supplied hypothesis. Positive scores are
uncalibrated proposal scores, not probabilities. At least one support and at most three
total distinct support/contradiction IDs are required. References are canonicalized in
ascending order and can cite only supplied native evidence, never hypotheses. TTL is
1..1,000,000 ticks, further capped by the earliest expiring source. With the same combined
reference set, repeats can only lower confidence/expiry; creation time stays fixed and
rewording errors. An exact duplicate statement/reference set under a new ID is rejected.
A currently valid hypothesis hidden by privacy or truncation cannot be overwritten unseen.
An expired/invalidated hypothesis ID can be reused with a different currently supplied basis;
the same-basis lifetime/confidence restrictions still apply. Changed evidence sets are new
proposals; the library cannot detect semantic paraphrases or prove that a citation entails a claim.

Stored hypotheses retain id/statement/confidence/created/expires/supports/contradicts/
privacy; snapshots additionally label them `tentative` or `contested`. No score or status
promotes a hypothesis to an observation or grants game-action authority. Full stores
replace the earliest retained hypothesis. `contradict(id,now)` makes an evidence source
unavailable. `forget_privacy` physically removes matching sources and every dependent
hypothesis; filtered snapshots alone do not physically erase persisted data.

Saved state includes version/subject/clock_id/revision/last_token/clock/max_hypotheses/
journal/evidence/hypotheses. Evidence metadata carries id/token/privacy/contradicted.
Restore requires expected subject/clock, validates counters, time/dependency/metadata
coverage and inherited privacy, and caps text at 1,048,576 bytes. This validates structural
consistency, not cryptographic origin. Persisted state is host-only and may contain
private facts; use filtered snapshots for model context. Privacy labels, authentic
observations, consent, clock progression and durable storage remain host responsibilities.

## Registered facts

`facts::FactRegistry` is an explicit host/user assertion store, separate from observations
and model hypotheses. Capacity is 1..256; full registration errors without eviction.
Identity is `(scope,key)`, each 1..64 non-control Unicode characters, at most 256 UTF-8
bytes. JSON values are typed, at most 4,096 serialized bytes, depth eight (root depth zero),
256 nodes and 64 elements/members per container. Object keys are unique and at most
256 bytes; strings at most 4,096 bytes. Number text is valid JSON, at most 128 bytes,
with explicit exponent magnitude at most 1,000,000. Values retain their number spelling.

`register(scope,key,value,FactOptions)` requires trusted host routing. Options are `now`,
`authority` (`host` default or `user`), `pinned`, optional `expires`, `privacy`,
`dependencies` and optional `expected_revision`. A new identity requires no expected
revision. Replacement requires the exact current revision; a pinned fact errors until
explicit `unpin`, and a host-authority update cannot replace a user-authority assertion.
The authority enum records caller-supplied provenance, not authenticated identity.

Successful registration returns `{scope,key,revision}`. Every registration, unpin,
ordinary deletion or nonempty privacy erasure advances a store-wide generation, bounded
at 2,147,483,647; exhaustion errors. Even unpinning an already unpinned fact advances its
revision. Mutation errors leave state untouched. Registration time is nondecreasing,
0..4,294,967,295; reads must not precede the last registration time. Reads do not advance
the clock. Expiry is absent or strictly after registration and at most 4,294,967,295;
an assertion is unavailable at `now >= expires`.

Dependencies are at most eight exact `FactRef`s, no duplicate scope/key or self reference.
Each must match a current, active fact. Replacing a fact using its own descendant is
rejected atomically. Reads recursively recheck source revisions, expiry and privacy;
changes invalidate descendants transitively without reviving old values or computing
new ones. Ordinary `forget` requires current revision, an unpinned fact and compatible
authority; dependent payloads remain stored but unavailable. Refresh them explicitly.

Native privacy input has at most eight distinct `[a-z][a-z0-9_]{0,31}` labels. Derived
facts inherit all dependency labels (sorted, deduplicated, at most 32). Read-denied labels
have the same grammar and a limit of 32; hidden, expired and invalid facts return no
value. `forget_privacy` is an explicit host erasure operation that physically removes
all matching payloads, including pinned facts and descendants with inherited labels.
This operation intentionally does not require ordinary unpinning/authority handling.

`check(scope,key,value,now,denied)` validates the proposed value and returns
`{status,reference}`: `supported` or `conflicting` with the current reference, or
`unknown` with null reference when absent or unavailable. Numeric equality normalizes
decimal coefficients/exponents without expanding large exponents or using floating point:
`1.00e0` equals `10e-1`, `-0.0` equals zero, and 9,007,199,254,740,993 remains distinct
from 9,007,199,254,740,992. Object order is ignored; array order and JSON types matter.
This establishes structural agreement with registered assertions, not independent truth,
logical entailment or natural-language equivalence. Model proposals cannot call a
registration tool by default; the library adds no such tool.

`packet(now,count,max_bytes,denied)` and `Turn::facts` emit
`{generation,columns,rows,dropped}`, with columns
`[scope,key,value,authority,pinned,revision,expires]`. Visible active facts sort pinned
first, then user authority, then scope/key ascending. Count is 0..256; bytes 1..16384.
Trim the tail until the entire compact object fits; if the empty header cannot fit,
error. Dropped counts only visible active facts excluded by count/bytes, never private,
expired or invalid facts. Packing does not remove stored facts or renew expiry.

Saved state has exactly `version:1,capacity,generation,clock,facts`; each fact has exactly
`scope,key,revision,value,authority,pinned,expires,privacy,dependencies`. Restore validates
unique identities/revisions, counters, typed value limits, older dependency revisions,
no self/duplicate dependency and inherited privacy for matching current sources. Missing
or stale sources remain representable as invalidated facts. Text loading is capped at
4,194,304 bytes. Structural validation does not authenticate persisted data. Full state
contains private and invalidated payloads: keep it host-only and filter model packets.

## Optional runtime upgrades

All seven APIs are opt-in native building blocks, with no new default model tools or
network/filesystem operations. JSON parsers reject duplicate keys. Runtime IDs, clocks
and revisions below use unsigned integers at most 4,294,967,295 unless otherwise noted.
Mutating clocks are nondecreasing; reads must not precede the last mutation and never
advance clocks. The host chooses a consistent tick unit and owns subject separation,
truth, consent, engine observations and current action permissions.

### Indexed recall

`RecallIndex::new(records, config)` owns an immutable copy of at most 4,096 `Record`s.
IDs are distinct/nonzero, turns bounded, importance 1..10, text <=280 Unicode characters,
tags <=8 brain-style names. Configuration has exactly `normalize` (boolean) and `aliases`
(up to 64 `[source,target]` pairs of brain-style names). Sources are unique, distinct from
targets, and no target can also be a source: chains/cycles are refused. Aliases apply once
*after* optional normalization, so supply normalized tokens as alias keys.

Tokenization lowercases ASCII and splits outside `[a-z0-9_]`, retaining tokens of 1..64
bytes. When normalization is enabled, strip `ing` from words longer than five bytes,
otherwise `ed` from words longer than four, otherwise a final `s` from words longer than
three that do not end in `ss`. This is intentionally a small lexical rule, not full
English morphology or multilingual semantic retrieval. Original text/IDs are unchanged.
Each record appears once in a term's posting list, even if the term occurs repeatedly.

`search(query,count)` limits query to 4,096 UTF-8 bytes and count to 0..128. Each distinct
query term adds one point per matching record. Rank by score, importance and turn
(descending), then ID ascending. Result is `{records,candidates,postings}` with record
fields `id,turn,importance,text,tags`; diagnostics count matched records before limiting
and posting visits. No match yields an empty list, unlike legacy recall's fallback.
Records must already be filtered for scope/expiry/privacy/provenance. Rebuild/drop the
index after edits, forgetting or permission changes; immutable snapshots cannot notice
external changes. There is no persisted index or implicit promotion into facts.

### Validated playbooks

A `Playbook` is a host-registered array of at most 64 recipes, sorted by ID on serialization:
`{id,action,args,requires,expires}`. IDs/action names use brain-style names; recipe IDs
are unique. Args must be an object within registered-fact value bounds (4,096 bytes,
depth eight/256 nodes). Requires has 1..8 distinct scope/key prerequisites, each exactly
`{reference:{scope,key,revision},value}`. Values have the same typed bounds. Expiry is
null or a positive bounded tick. Text loading and total serialized definitions cap at
524,288 bytes; this is definitions data, never executable scripts.

`select(facts,now,denied,validate)` returns matching `{id,action,args}` drafts in ID order.
Every prerequisite must be currently visible, structurally supported, and have the exact
registered revision; equal values under new revisions still invalidate the recipe. Expiry
and transitive source invalidation apply. Only then is the current host
`validate(action,args)` callback consulted, checking actual schema, permissions, resources
and game rules. False excludes the draft. The host must validate authored procedures and
recheck immediately before executing returned drafts. Matching does not prove effectiveness,
make calls, execute actions or accept model-authored procedures as trusted skills.

### Observable outcomes

`OutcomeTracker::new(capacity)` accepts 1..128 transient trials. `begin(id,now,wait,window)`
requires strictly increasing nonzero native IDs, wait/window 1..1,000,000, enough capacity,
and `now+wait+window` within the clock bound. Phases are planned→executed→exposed→observed;
`executed`, `exposed` and `resolve` require the preceding phase and `now < deadline`.
Exposure starts a single response deadline `now+window`; repeated exposure is refused.
`resolve` accepts one host score in integer millionths 0..1,000,000. The host establishes
an actual opportunity to observe a response; silence without observation is not failure.

`report(id,now)` emits `{id,status,score,trainable}`. Unobserved phases past deadline, and
cancelled trials, are unknown with null score/false trainable. Observed scores remain
available, including observed zero. `take_label` returns an observed report once and
marks it consumed; later reports remain observed but untrainable, and repeated take errors.
`cancel` refuses terminal trials. `forget` frees a slot without resetting the ID watermark.
The host joins labels to original action/features and validation sets; no automatic
weight update, causal claim or fear/personality inference follows from a score.

### Temporal event features

`TemporalWindow::new(capacity,window,start)` has capacity 1..256 and window
1..1,000,000 ticks. The host asserts observation coverage from start. Native
`observe(token,now,expires,kind,value)` requires increasing nonzero tokens, brain-style
kind, expiry after now, value in −1,000,000,000..1,000,000,000 and current availability.
Prune expired/out-of-window events before a successful insertion. A full live window
refuses insertion atomically, rather than silently undercounting; watermarks survive
pruning and availability changes. Repeated reads do not create events.

At query time retain `event.at <= now < expires` and `now-event.at < window`.
`features(kind,now)` returns `{count,rate_millionths,first,last,delta}`. Rate is
`floor(count*1,000,000/min(window,now-start+1))`; first/last are retained event times,
delta is last numeric value minus first (null if no samples). This is event frequency,
not a probability. `sequence(kinds,now)` accepts 1..8 names, greedily counts ordered
non-overlapping subsequences; gaps are allowed, and distinct same-tick events may match.
A sample is used at most once in one sequence match.

Changing `availability` clears retained events and resets coverage start. Unavailable
queries return null count/rate/sequence; available empty coverage returns zero.
Hosts must clear availability on observation gaps, subject/world changes, privacy
revocation or source erasure. No sensor polling, archive, habit certainty or preference
training is implied. Outcome/temporal trackers are transient, not restored from saved
JSON; hosts retain durable source watermarks if their engines can replay old events.

### Shared perception frames

`BrainBundle::new(perception,decision,ports)` owns two independent existing brains.
Ports is 1..16 `{input,output}` bindings, unique by decision input, pointing from a
perception output to a decision input. All signals are normalized millionths; host
`FeatureSchema` bindings still define physical units. Bounds apply per module (up to
32 nodes/16 inputs/16 outputs each); modules are not cyclic or mutually recurrent.

`prepare(scope,frame,inputs)` validates perception inputs and computes one sealed
`PreparedFrame`. Scope has 1..64 non-control Unicode characters; frame is nondecreasing.
Each successful prepare invalidates prior frames, including same-frame/same-scope ones.
`run_into` requires the originating bundle, exact scope/frame, full decision input array
with imported slots None, current output mask/thresholds and a caller-owned Prediction.
Imported scores are filled from the sealed frame; unknown scores remain unknown.
JSON `run` requires exactly the non-imported input keys; imported overrides are refused.
Failed native runs preserve the output object.

Clone a module, teach/train it through existing guards, then call `replace_perception`
or `replace_decision`. Port compatibility is checked atomically; successful replacement
invalidates prepared frames while preserving the other module's weights/history/pins.
Native candidate inference allocates no new buffers; prepare allocates a new sealed
identity/scope once per cycle. Saved bundle is exactly
`{version:1,perception:<brain-state>,decision:<brain-state>,ports}` with text cap
528,384 bytes. Restore revalidates both modules and ports, creating a new identity;
prepared frames are never persisted or shared across actors/cycles.

### Asynchronous result tickets

`RequestContext::from_value` accepts exactly `{actor,world,schema,permissions,facts}`.
Actor/world use scope strings; schema/permissions are host revision counters. Facts has
at most 16 exact `FactRef`s, unique scope/key and canonicalized by scope/key. Include
all relevant dependencies; undeclared state changes cannot be checked automatically.

`RequestGate` holds at most 32 pending tickets. `issue(context,now,ttl)` creates an opaque
instance-bound ticket with monotonic ID, TTL 1..1,000,000 and bounded expiry. Native
`accept(ticket,current,now,facts,denied)` returns true only before expiry, with exactly
matching current actor/world/schema/permission context and visible current fact revisions.
It consumes the ticket even when freshness checks return false. Unknown/replayed/foreign
tickets error. Invalid clocks/denied labels error before consumption. `cancel` explicitly
frees a slot; expired slots remain until accept/cancel. Tickets/gates have no saved form;
restart invalidates outstanding requests. Context parsing never creates an executable
capability. A true result is a freshness check, and action/schema/permission validation
still belongs immediately before native execution.

### Runtime spending ledger

`SpendingLedger::new(limit,window)` uses integer microdollars (1,000,000 = $1), limit
0..1,000,000,000,000 and window 1..1,000,000 host-clock ticks. At most 256 charges are
retained; at most 256 simultaneous reservations, sharing one cap. `reserve(now,amount)`
prunes only expired settled charges and checks spent+reserved+amount against the limit.
Pending charges never age out. IDs increase across cancellation/recovery. Every retry
requires its own reservation; the caller supplies provider pricing and maximum input,
output/reasoning/retry costs. Unknown pricing is not inferred by the library.

State transitions are reserved→dispatched→settled. `cancel_before_dispatch` refunds only
reserved requests; dispatched requests cannot be cancelled for a refund. `finish` takes
actual cost or None for unknown usage (full reservation retained), and starts settled
retention at finish time. If actual cost exceeds reservation, record it and permanently
block positive-cost reservations until the host explicitly replaces/audits the ledger;
zero-cost fallback remains possible, within entry limits. A cap cannot prevent an
already-dispatched provider from billing more than its estimate.

Saved state is exactly `{version:1,limit,window,clock,next,blocked,entries}`; charges are
`{id,at,amount,state}` in increasing ID order. `summary(now)` emits `{spent,blocked,pending}`
without changing clocks or retention. Restore validates fields, counters, ordering, time,
costs and total consistency, then `recover(now)` settles *all* unfinished reservations
at full cost. Text cap 65,536 bytes. Recovery therefore deliberately changes pending
state; a second same-time restore is stable. Durable atomic saving before dispatch,
crash exclusion, provider request caps and authenticated billing remain host duties.
