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
   identifier segment split at `:`, `.`, `/` or `-`, or an `_`-separated part of
   such a segment, lowercased. Full cells and full segments are retained too. Rows with
   the most matching words are kept; if no
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
  match their ID, its identifier components split at `: . / - _`, or their tags
  (also split into those components, without `#`; a trailing plural `s`
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

## Optional query guidance

`query::Guidance` defaults to `Standard`, whose `text()` is empty. The opt-in
`ExplicitSelection` profile returns the exact supplemental system paragraph in
`query-guidance/explicit-selection/expected.json`. Append it once to the system prompt
beside table descriptions; keep it for correction/final-answer turns. `Guidance::parse`
accepts exactly `standard` and `explicit-selection`; other names are errors. CLI
`query --guidance PROFILE` prints its text as a JSON string. No model-name/size detection
or query rewriting is performed; default tools, descriptions and sessions are unchanged.

The conformance `query-guidance` operation takes a JSON profile string and returns the
supplement as a JSON string. Unknown names and non-string input fail. The profile trades
more input for a mixed per-model result; see the [paired report](QUERY_OPTIMIZATION_2026_11_06.md).

## Query plans and host contracts

`query::Query::plan()` and `gamectx query --explain SQL` return this structure without
reading a table (example is `query-plan/top-n-plan`):

```json
{"table":"cargo","aggregate":true,"columns":["mass"],"filters":[],"order_by":{"column":"mass","descending":true},"limit":3}
```

`columns` lists distinct names in first-use order: SELECT expression traversal, WHERE
conditions, then ORDER BY. `aggregate` means any selected expression uses an aggregate.
`filters` keeps query order, each `{column,op,value}` with `op` one of `= != < <= > >= in`;
`<>` becomes `!=`. IN values are arrays. Numeric filter literal spellings are retained.
`order_by` and `limit` are null when absent. Aliases are accepted but not represented.

An optional `query::Shape::from_value` reads exactly
`{table,filter_columns,limit,aggregate}`. Names are ASCII SQL identifiers of 1–64 bytes;
`filter_columns` contains at most 16 names. Duplicate names are permitted to express
multiple conditions on one column. `limit` is null or a canonical unsigned integer
0..4294967295; `aggregate` is boolean. Unknown/missing fields and wrong types fail.
`to_value()` sorts filter names, preserving duplicates, in the displayed field order.
`Query::require_shape` requires the exact table, aggregate flag, limit and multiset of
filter columns; WHERE order can differ. It does not check values/operators, selected
expressions, sort order or authorization. Hosts supply these contracts independently
when they know the required structure; they are not inferred from natural language.

`run_bounded(rows,max_rows,result_bytes)` rejects an oversized table before execution,
and an oversized compact UTF-8 JSON result after computation. It never truncates.
These bounds do not limit cell sizes, transient allocations or elapsed time. Ordinary
`run`, default tools and session envelopes retain their existing behavior.

The conformance `query-plan` operation takes `{sql,shape?,op?}`; default `op:"plan"`
returns the plan. With `op:"run"`, `rows`, `max_rows` and `result_bytes` are required,
and output is `{plan,result}`. This operation is a test adapter for the native APIs,
not a second model query language or new tool schema.

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

## Model measurement references

The [fresh alpha.3 model check](MODEL_TEST_2026_11_06.md) uses these existing table,
metadata and query-session formats without changing them. Across six synthetic states,
full JSON and packed tables each scored 27/96; schema + query scored 91/96. Average
completed-exchange input was 1,786.792 / 1,252.125 / 916.490 tokens. All 105 recorded
query feedback envelopes match Rust and Python exactly. Syntax/shape validation still
cannot establish that a valid query matches a natural-language intention; the report
records four syntactically valid queries with wrong intent and one terminal syntax failure.

## Integration kit

Opt-in version 1; existing formats are unchanged. `kit::process` / `gamectx kit` accept
exactly `{project,op,args}`. `Project::parse` accepts the project alone. Unknown, missing
and duplicate fields are rejected. Full workflows and examples: [KIT](KIT.md).

A project has exactly `version:1`, `datasets`, `tools`, `recipes`. Maximum compact project
size 262,144 UTF-8 bytes; at most 16 datasets, 32 tools and 32 recipes. Names (including
field/property/section names) are lowercase ASCII identifiers, initial letter, at most
32 bytes. Dataset names `tools` and `schemas` are reserved. Names must be unique in their
registration list. Descriptions forbid controls, with 280 Unicode characters for datasets
and fields, 1,024 for tools and parameter schemas. Nonempty units allow 32 characters.

| Registration | Exact fields |
|---|---|
| Dataset | `name`, `description`, `id` (column name), `max_age` (null or unsigned u32 ticks), `fields` (1..64) |
| Field | `name`, `type` (`string`, `number`, `integer`, `boolean`), `description`, `unit` (text or null), `nullable` (boolean), `visibility` (`model` or `host`) |
| Tool | `name`, `description`, `parameters` (supported object schema) |
| Recipe | `name`, `budget_bytes` (0..1,048,576), `datasets` (0..16), `tools` (unique declared names, 0..32), `sections` (0..32) |
| Recipe dataset | `name`, `mode` (`rows` or `schema`), `view`, `priority` (integer −1,000,000..1,000,000), `required` (boolean) |
| Recipe section | `name`, `priority`, `required` |

The ID field must be non-null model-visible string. Views allow only `fields`, `where`,
`sort_by`, `limit`, with existing view semantics. All referenced columns must be visible.
Explicit fields must be nonempty and unique; limit is unsigned 0..4,096; IN arrays hold
at most 128 options. Recipe dataset/section names cannot collide or use reserved names.
Maximum ages, versions, limits, counts and times use canonical unsigned integer text.

A snapshot is exactly `{now,tables,sections}`, at most 1,048,576 bytes. `now` is unsigned
u32 ticks. `tables` maps registered dataset names to exactly `{updated_at,rows}`; capture
time is at most now. Each table has at most 4,096 rows with exactly its declared columns
and matching scalar/null types. IDs are unique strings of 1..256 Unicode characters.
`sections` maps at most 32 names to trusted arbitrary host JSON values. Unknown tables,
invalid rows and future captures fail before visibility projection. Selected stale tables
fail before building or querying. Host-only fields are removed from the retained snapshot.

Tool schemas support only `type`, `description`, `enum` and the following type-specific
keywords. Type is one supported JSON type or `[type,"null"]`. Root parameters must be a
non-null object. Maximum schema depth 8, 256 nodes. Object properties: at most 32; strings
count Unicode scalar characters. Unknown keywords, including refs, patterns and unions
other than nullable types, are refused. Schema enum values must satisfy their schema
and be distinct under recursive JSON equality with exact decimal numeric equality.

| Type | Keywords and requirements |
|---|---|
| object | `properties` object, `required` unique subset, `additionalProperties:false`, all mandatory |
| array | `items` schema and `maxItems` (0..128), mandatory; optional `minItems` (0..max) |
| string | mandatory `maxLength` (0..4,096); optional `minLength` (0..max) |
| number, integer | optional `minimum`, `maximum`, ordered exact decimals |
| boolean, null | no additional keywords |
| any supported type | optional `enum` (1..32), `description` |

Dataset/tool numbers are finite as f64 and use at most 128 bytes of JSON number text,
with exponent magnitude at most 10,000. Integer validation and tool bounds do not round
through f64; decimal text is retained. This does not change view/query numeric semantics.
Arguments and callback results are independently capped at 65,536 bytes, including JSON
framing. Callback result validation occurs after execution and provides no rollback.

Stable packet sections are `tools` then `schemas`, when nonempty, both required with
priority 1,000,001. Tools are ordinary function wrappers with `strict:false`. Schema entries
are keyed by dataset and contain `description` and `columns`; each column contains
`name,type,description,unit,nullable`, in recipe projection order. Dynamic packed table
sections follow recipe order, then supplied extra sections; existing budget receipt and
dropping rules apply. `schema` mode includes no dynamic table. Empty row packing uses the
existing empty-table format, while declared schemas remain present.

Build returns `{packet,inspection}`. Inspection is host-only, exactly
`{recipe,receipt,datasets,missing_sections}`. Each dataset report contains
`name,source_rows,selected_rows,source_indices,fields,omitted_fields,updated_at,age,mode,view,included`.
Indices refer to the original input table order, before selection. Schema-mode datasets
are included; row mode reports whether its dynamic section survived. Omitted field names
include private fields. This diagnostic object is outside the model packet budget.

Dispatcher success is `{ok:true,result}`. Failure is
`{ok:false,error:{code,message}}`; codes are `unavailable_tool`, `invalid_arguments`,
`missing_handler`, `host_error`, `result_limit`. No callback executes before recipe and
argument validation. Host error details are not copied into model replies. Callback
execution, live authorization and idempotency remain host responsibilities.

Persistent/native bridge requests are exactly `{version:1,id,op,args}`. ID is nonempty
text, at most 64 characters. Reply order is `version,id,ok,result` on success, or
`version,id,ok,error` with code `invalid_request`. ID is null when framing/version/ID
validation fails; otherwise the original ID is echoed. Messages are diagnostic and not
an exact error-text contract. Each line/request is at most 1,572,864 bytes; the server
drains an oversized line in bounded chunks and continues. A line above twice the cap
plus two framing bytes terminates serving to bound recovery work. EOF completes a partial final
line. Malformed JSON, invalid UTF-8 and unknown versions/operations get error replies.

| op | Exact args / result |
|---|---|
| build | `{recipe,snapshot}` → `{packet,inspection}` |
| query | `{recipe,snapshot,sql}` → ordinary query result over the recipe view, including its row/column restrictions; 4,096 rows / 65,536 result bytes |
| validate | `{name,arguments}` → validated `{name,arguments}`, without executing or authorizing an action |
| doctor | `{}` → `{version:1,datasets,tools,recipes,warnings}`; warnings `{code,path}`, codes `unbounded_age`, `missing_unit` |
| tools | `{recipe}` → selected function schema array |
| features | `{dataset,revision,policies}` → existing brain feature schema; policies contain `name,min,max,invert,missing,outside`; registered units are inserted |
| evaluate | the suite below → `{version:1,runs:[reports]}` |

Feature conversion retains the existing brain limits, including lowercase names, numeric
ranges, revision 1..1,000,000 and at most 16 features. It refuses nonnumeric/private fields
and missing units. Policies are explicitly host supplied, not inferred.

An evaluation suite has exactly `{version:1,cases,runs}`, at most 1,572,864 bytes, with
1..128 cases and 1..8 runs. Cases have exactly `id,recipe,snapshot,question,expected`;
IDs use the identifier rule, questions are nonempty control-free text up to 4,096
characters. Runs have exactly `name,recipe,guidance,observations`; recipe is null (use
case recipe) or an override; guidance is a registered query profile name. Each observation
has `case_id,sql,answer,input_tokens,output_tokens,latency_micros,tool_calls`. SQL is null or
at most 4,096 bytes; usage/latency are null or unsigned integers at most 1,000,000,000,000.
Tool calls are at most 32 `{name,arguments}` objects. Every run must contain every case
exactly once; extra/duplicate/missing observations fail.

Each report has `name,guidance,cases,answers_correct,queries_correct,invalid_queries,
invalid_tool_calls,packet_bytes,input_tokens,output_tokens,latency_micros,details`.
Details are `{id,answer_correct,query_correct,invalid_tool_calls,packet_bytes}` in observation
order. Counts/bytes/usage sum over the cohort; one missing measurement makes its total
null. No SQL means no query success, not an invalid query. Invalid tool names or arguments
count without dispatch. Expected-result equality ignores object key order but preserves
array order and exact number spellings. The scorer does not verify external usage, apply
guidance or contact a provider. `question` is for the collecting host harness.

## Local-model workflow contracts

All are opt-in additions. Existing kit v1 and legacy packet/query/session shapes remain
unchanged. `gamectx local <workflow.json>` accepts `-` for bounded stdin; `kit` bridge op
`local` passes the same workflow. A workflow is exactly `{kind,args}`, maximum 1,572,864
UTF-8 bytes. This adapter's dialogue/scene counter counts request JSON bytes and labels
that count `estimate`; it is a deterministic fixture counter, not a model tokenizer.
Production Rust callers supply their complete rendered-request counter.

| kind | Exact args | Result |
|---|---|---|
| budget | `{budget:{context,output,safety},count:{input,method}}` | request receipt |
| dialogue | `{thread,current,protect,maximum,tokens}` | selected messages and kept/omitted exchange IDs |
| scene | `{scene,protect,maximum,dialogue_tokens,budget}` | budgeted packet/request/section receipt plus dialogue selection |
| decision | `{choices,calls:[{text,revision,available}]}` | `{schema,results,finished}`; available is a host-supplied list of current legal pairs |
| cancel | `{choices,available}` | live-approved fallback or abstention, without a model attempt |
| preset | `{size,name,identity}` | `{guideline,profile}`; advisory size class and editable existing v1 definition |
| profile | `{profile,actual_identity}` | `{identity,guidance,transport,options}` after complete identity comparison |
| report | `{version:1,trials:[...]}` | `{version:1,conditions:[...]}` complete-turn telemetry |

Request budgets require canonical unsigned `context` (1..1,048,576), `output`, `safety`;
output+safety cannot exceed context. Counts are canonical unsigned 0..1,048,576 with method
`exact` or `estimate`. Receipt fields in order: `context,input,reserved_output,safety,
remaining,overflow,method,fits`. Remaining/overflow saturate at zero; assess reports an
overflow, while dispatch checks/builds refuse it. Renderer/counter failures are errors.
Assembly accepts at most 128 sections and bounded request JSON. `BudgetedRequest::to_value`
is `{packet,request,sections,budget}` using the legacy section receipt as `sections`.
The host must enforce generation limits and provide an honest full-render count.

Threads are exactly `{version:1,scene,conversation,participants,exchanges}`, at most
262,144 bytes. Scene/conversation/request/participant IDs are nonempty, control-free,
up to 64 Unicode characters / 256 bytes. Participants are 1..16 unique IDs. There are
0..128 exchanges, each exactly `{id,turn,user,assistant}` with unique IDs, strictly
increasing unsigned u32 turns and nonempty user/assistant text up to 4,096 UTF-8 bytes.
Message text may contain quoted controls represented by JSON escapes. Protect is 0..3;
maximum is 0..128 and at least protect. Selection returns exactly `{messages,kept,omitted}`;
messages are alternating `{role,content}` pairs. The current request ID is excluded. Empty
dialogue emits no messages. A protected tail that cannot fit is an error. Compaction
candidates are existing whole exchanges; the API never deletes or invents history.

Scenes are exactly `{version:1,id,rules,current,outcome,required_facts,dialogue,optional}`.
ID must equal the thread's scene ID. Rules are nonempty text up to 8,192 bytes; current is
exactly `{id,text}`, text nonempty up to 8,192 bytes. Required facts are an object of at
most 64 entries; outcome is host-supplied JSON. Optional sections are at most 32 objects
`{name,priority,value}`, with unique scoped names, priorities −1,000,000..1,000,000 and
no reserved names (`rules,scene,facts,outcome,dialogue,input`). Rules/facts/outcome/input/
scene and the protected dialogue are required. All optional sections drop before older
unprotected dialogue; optional data is not restored ahead of dialogue. Scene output is
`{packet,request,sections,budget,dialogue}`. `render_chat` uses native roles and JSON scene
data, not manually nested chat-template markers.

Choices are exactly `{version:1,revision,choices,fallback}`, at most 16,384 bytes. Revision
is unsigned u32; choices are 1..32 distinct pairs `{action,target}`. Action is a lowercase
ASCII identifier of 1..32 bytes, initial letter; target is null or a scoped ID. Fallback
is null or a declared pair. Schema is an ordinary closed object schema with properties,
required fields and whole-pair `enum`. A model response must be strict JSON at most
4,096 bytes, with exactly action and target and one declared pair. No code-fence/prose
repair occurs. The workflow allows at most three calls to demonstrate terminal-session
rejection; the actual session permits one correction only. Stale revisions bypass retry.
Replies are `{status:"accepted",decision}`, `{status:"retry",error:"invalid_decision",schema}`,
`{status:"fallback",decision}` or `{status:"abstain",decision:null}`. Availability checks
and final atomic execution are host responsibilities. The library invokes availability checks; action execution stays outside this API.

Identity has exactly `model_digest,quantization,template_hash,tokenizer,runner`, nonempty
control-free text up to 128 Unicode characters each. Identity normalizes key order in
that order and compares all values. A profile is exactly
`{version:1,name,identity,budget,threads,transport,guidance,temperature_milli}`; name is a
scoped ID, threads 1..64, temperature_milli unsigned 0..2,000. Transport is `json`, `schema`,
`tools` or `signatures`; guidance is `standard`, `concise`, `example`, `explicit-selection`.
Size presets accept `1`, `1.5`, `3`, `8`, `30`, `70` with optional `b`/`B` suffix. They return
a guideline with size class, nominal parameter millions, task/content/choice hints,
context/output starting values, host-work notes and `measured:false`; no library enforcement
is inferred from a hint. Other models can use an ordinary explicit profile.
Options are `num_ctx,num_predict,num_thread,temperature`, with temperature written as a
minimal decimal scaled from thousandths. Settings are returned, never applied to services.
Standard adds no supplemental text. The exact text is defined by the new conformance
cases, without changing the existing query guidance format or default.

Turn reports accept 1..512 trials and at most 1,572,864 bytes. Each trial has exactly
`id,condition,expected,actual,elapsed_micros,ttft_micros,attempts,fallback,required,supplied,
memory,dialogue_score`. IDs are unique scoped IDs; condition is a scoped name. Elapsed is
unsigned 0..1,000,000,000,000; TTFT is null or within elapsed. Fallback is boolean; required
and supplied are unique scoped-ID arrays of 0..64. Dialogue score is null or unsigned 0..5.
Expected/actual equality ignores object key order but preserves array order and exact
number spellings. The report does not certify semantic truth or external measurements.

Each of at most eight attempts has exactly `phase,input_tokens,output_tokens,cached_tokens,
prompt_micros,decode_micros,load_micros,valid,truncated`. Metrics are null or unsigned
0..1,000,000,000,000; flags are booleans. Phases `decision,query,narration` occur at most
twice each; `preflight` at most four times. Retry count excludes preflights and counts
repeated model phases. Intentional one-token preflight truncation is excluded from the
truncation count, while preflight time/tokens still contribute to totals.

Memory has exactly `runner_rss_bytes,game_rss_bytes,available_bytes,swap_in_bytes,
swap_out_bytes`, each null or unsigned 0..9,007,199,254,740,991. Reports group conditions
in first-use order and return name/trial/correct/invalid-attempt/retry/fallback/truncation/
missing-required counts, integer median and nearest-rank p95 turn microseconds, TTFT
median and coverage, summed usage/timings, memory extrema and dialogue-score coverage/
total. Empty or partly unknown usage totals remain null. Memory is max for RSS/swap and
min for available RAM; any unknown trial makes that metric null. No historical evaluator
format was extended to accept these fields.

## Optional optimization workflows (v0.3.0a)

The existing `local` CLI and kit bridge retain their envelope and old kinds. New kinds
are additive; all v0.2.0a conformance expectations remain unchanged. Input is still
`{kind,args}`, with the existing 1,572,864-byte work bound.

| kind | Exact args | Result |
|---|---|---|
| intent | `{choices,intent:{request:{id,text},facts,allowed},calls}` | narrowed schema, guarded decision results, finished; retry adds correction data |
| query-template | `{template,arguments,rows}` | `{tool,result}` from typed AST-bound predicates |
| evidence | `{records,max_bytes}` | `{packet,kept,omitted,bytes}` with complete dependency closures |
| compose | `{definition,observations}` | `{status,sequence,candidate,budget,omitted}` host-count handshake |
| route | `{choices,revision,candidates,available}` | `{route,reason,decision,trace}`; no world execution |
| choice-codec | `{choices,names,calls}` | `{offer,schema,results,finished}`; input uses `{choice:name}` |
| tune | `{identity,cases,candidates,limits}` | split reports, Pareto frontier and optional exported tuning winner |

`choices` and `calls` keep their existing decision definitions. `intent.allowed` is a
host-defined subset, not an inferred language interpretation. `query-template` is a host
adapter over registered queries, not an additional model query language. `available` is
host-supplied current availability. All operations are pure except session state in the
native Rust APIs; no adapter operation invokes game callbacks.

Full required fields, bounds, validation/selection order, byte versus token semantics,
model-facing versus host-only results and failure behavior are specified in
[OPTIMIZATION](OPTIMIZATION.md). In particular, composition acknowledgement equality
preserves JSON object order and number spelling; a count for another candidate cannot
be used. Ready receipts still trust the host counter. Missing tuning metrics remain null
and exclude selection. Held-out results do not select configurations.

## Dungeon example protocol (v0.3.0a)

The standalone `dungeon` Cargo example defines a bounded version-1 JSON-lines host and
`{seed,commands}` replay format. This is an example protocol, separate from the unchanged
library/kit/local formats. Its action/visibility/revision rules and request limits are
specified in the [testbed guide](../examples/dungeon/README.md). `examples/dungeon/cases`
contains separate example conformance fixtures for the pure replay rules. They do not
add a library operation or require library ports to implement the game.


## Task workflow templates and direct rendering (v0.3.0a)

Two additive `local` kinds preserve existing kinds and expected outputs:

- `workflow`, args exactly `{size,task,name,identity}`: `size` uses the existing preset
  aliases, `task` is `interpret`, `dialogue`, `narration` or `query`. Returns the unchanged
  generic `guideline`, a v1 `profile` with task output reservation, and `workflow`
  metadata with version 1, task, size_class, measured:false, renderer:direct,
  response:choice/speech/sql, exchanges_hint, recall_hint, choices_hint,
  include_quest_goal:false, required_policy:preserve_or_fail and
  hint_policy:editable_not_capability_limits. No configuration or content is applied.
- `render-direct`, args the packet itself: `rules` string; `input` exactly `{id,text}`
  strings; `dialogue` an even-length array of at most 256 exact `{role,content}` objects,
  alternating user/assistant. Other fields become compact context JSON in insertion
  order. Returns `{messages}` with the rules as native system content, supplied dialogue,
  and a final user message `Context data:\n<JSON>\n\nCurrent request:\n<text>`.
  Bounded input 1,572,864 bytes; no template/schema/tool framing is inserted.

The Rust APIs are `deployment::{Workload,WorkloadHints}`, `ModelSize::workflow_profile`,
`ModelSize::workload_hints` and `dialogue::render_direct`. Thirty new conformance cases;
all 565 previous cases unchanged. [Profiles and measurements](WORKFLOWS.md).

The example-only dungeon adds `interpret`, `workflow`, `speech_prepare`, `speech_finish`,
`speech_state` and `speech_forget` to its own JSON-lines protocol. Its social replays and
fixtures live under `examples/dungeon`, outside the core format corpus. Protocol arguments,
scoped memory, clarification revision checks and bounds are specified in its
[guide](../examples/dungeon/README.md). These host operations are not automatic model tools.


## Shared conversation, reference and confirmation contracts (v0.3.0a)

Local kinds `conversation`, `references` and `confirmation` expose the additive
native APIs `dialogue::{Conversation,ConversationLimits,resolve_references}` and
`decision::PendingAction`. Exact input/output schemas, bounds and workflow operations
are specified in [CONVERSATION](CONVERSATION.md). Thirty-eight new cases under
`conformance/conversation` cover these operations; all 595 prior cases stay unchanged.
The replay adapters create fresh state; native callers retain instances across turns.
No adapter dispatches game actions or promotes generated speech into authoritative facts.

The example-only dungeon JSON host adds `trade` with exactly `{speaker,input}` and
`trade_cancel` with `{}`. Trade returns `{handled:false}` for dialogue fallback or
`{handled:true,reply,offer?,result?}` for a local receipt. Offer is the shared confirmed
proposal contract; result is an accepted deterministic game action. A pending offer is
created only by the host's explicit purchase-request handler, never by generated prose.


The user-feedback corpus `eval/dungeon-feedback.json` is diagnostic data, outside the
core conformance formats. It preserves unpinned reported input/reply text and separate
manual annotations; missing deployment/world/usage data remain null. Its offline audit
reproduces host rules and feeds supplied speech through validation, without treating
structural acceptance as semantic accuracy. [Report](DUNGEON_FEEDBACK.md).


## Authored lore and source-bound answers (v0.3.0a)

Additive local kinds `lore` (`{world,select}`) and `answers` (`{definition,calls}`) expose
`lore::Lore` and `dialogue::Answers`. [LORE](LORE.md) specifies exact fields, byte/count
bounds, audience/grant checks, required-fit semantics, source precedence and response
validation. Twenty-eight new conformance cases leave all 633 previous cases unchanged.
Authored lore is not live state; quote answers preserve player words with attribution.
The adapters select/render data and never dispatch actions or register model text as facts.

The example dungeon adds optional `--world FILE` and `--dialogue sources|legacy` and
host operations `knowledge_prepare`, `knowledge_finish`, `lore_speech_prepare`. Old
speech/observation/replay formats stay unchanged. Only the new host-rendered factual
path permits up to 512 reply characters; legacy generated speech remains capped at 240.


## Informational tools and structured lore (v0.3.0a)

Additive local kinds `info` and `inventory` implement the exact contracts in [INFO.md](INFO.md).
`info` args are `{scope,calls:[{name,text,sources}]}` (at most 128 calls), returning
`{scope,notes,results:[{ok:true,result}|{ok:false}]}`. `inventory` args are
`{snapshot,calls}` (at most 64), returning `{snapshot,results}` with the same isolated
call-result envelope. Calls use `op:rows` with `owners,tags`; `op:lookup` with `text,owners`;
`op:transfer` with `revision,moves,allow`; or `op:propose` with `owner,text,allow`.
The CLI's exact `allow` list is host policy, never a model-provided capability. It is at
most 32 entries. No outer state is changed if initial validation fails.

Lore version 2 is additive: `value` replaces `text`, with at most 4096 serialized bytes
per value. Selection packets preserve that field. Mixed-version fields are rejected.
All version-1 lore output remains byte-identical. See [LORE.md](LORE.md).


## Conversation tool workflow (v0.3.0a)

Additive local kind `interactions` accepts `{scope,actor,participants,operations}` with
at most 128 operations. An operation is exactly `{op,args}`: host operations `event`,
`progress`, `references`, `offers`, `respond`, `clear`, or model-dispatch operation `call`.
Call args are `{name,text,sources,revision,max_bytes,allowed}`; `allowed` is a host list
of at most 32 exact decisions. `respond` args are `{recipient,revision,confirm,terms,allowed}`.
`clear` args are `{}`. Other host records and all model arguments follow
[INTERACTIONS.md](INTERACTIONS.md). Results are `{state,results}`; per-operation results
use `{ok:true,result}` or `{ok:false}`. State is private host inspection, not a model packet.
Invalid initial configuration rejects the workflow. Model-call failures are atomic;
recipient confirmation follows the existing one-attempt consumption contract.

## Actor-visible location and world lookup (unreleased v0.3.1a)

These additive formats preserve existing outputs. Location definition fields are
`version:1,revision,places`; each place has `id,label,summary,description,details`.
IDs/labels are scoped strings of up to 64 characters. Summary/description are null or
nonempty text up to 512/2048 characters, without controls. At most 128 places; `details`
is an object of at most 32 fields/16,384 bytes. Definitions are at most 1,048,576 bytes.

Model arguments are exactly `{detail:"brief"|"normal"|"full"}`. Current place,
visibility, revision and byte cap are host inputs. Result field order is
`revision,location`; location fields are `id,label,summary`, then `description` unless
brief, then `details` only for full. Denied/missing location is null. Requested detail
must fit whole; no automatic truncation. The `look_around` facade uses this same format.

World definition fields are `version:1,id,revision,states`; each state has `key,value`.
At most 64 unique scoped keys; each arbitrary JSON value is at most 4,096 bytes; the
whole definition is at most 524,288 bytes. Model arguments are exactly `{keys:[...]}`,
with one to eight unique scoped keys. Result field order is `id,revision,states`; state
keys follow requested order. Denied/missing keys are absent; authorized unknown is null.
World IDs, revisions and visible keys are host-bound. No calendar is assumed.

Both raw APIs accept output caps 64..32,768 bytes and reject stale revisions. Character
calls narrow their result cap to 64..8,192 bytes. Definitions and calls use the existing
strict JSON parser. Details/values must be filtered by the host for each actor/audience.

New local workflow kinds are `location` (`definition,text,current,revision,max_bytes,available`),
`world` (`definition,text,revision,max_bytes,available`), `quote-phrase` (`quote`) and
`character-plan` (`scope,actor,participants,view,turn,budget`). The last is an offline
identity-renderer/one-byte-counter fixture, not a provider tokenizer. Its result order
is `request,selected_tools,input_bytes,omitted`. Fifty new fixtures specify exact bytes.
Quote phrasing is a new opt-in renderer; old answer-resolution output stays unchanged.

The new version-1 character host operations and view/plan shapes are documented in
[CHARACTERS.md](CHARACTERS.md). Persistent bridge envelopes retain their prior shape.

## Routines, observations and time knowledge

Additive unreleased v0.3.1a formats; all previous expected outputs stay unchanged.
See [ROUTINES](ROUTINES.md) for policy/host responsibilities and persistent operations.

Routine definition, exact fields: `version:1,id,revision,calendar,activities`. At most
128 activities; 131,072 definition bytes. Scoped IDs are nonempty, no controls, at most
64 Unicode characters/256 UTF-8 bytes. Revision/tick/reading coordinates are canonical
unsigned JSON integers through u32::MAX. Every activity has exactly:

`id,location,start,end,repeat,priority,requirements,interrupt,resume,missed,grace`.

IDs are unique; `start<end`; `repeat` is null or 1..1,000,000 ticks and at least window
duration. Priority is 0..1,000,000. Requirements are at most 16 unique scoped IDs.
Interrupt is `allow|locked`, resume `resume|cancel`, missed `cancel|defer`.
Grace is 0..1,000,000, and must be zero for cancel. Host conditions are not executable text.

Configuration accepts partial default/override objects; unknown/duplicate fields fail.
The resolved serialization uses this order:

| Field | Default | Accepted bound |
|---|---|---|
| `routines,history,time` | false each | boolean; time requires history |
| `tools` | [] | unique subset of `routine_status,activity_propose`; requires routines |
| `timeline` | 32 | 1..128 |
| `observations` | 32 | 1..128 |
| `retention` | 1000 | 1..1,000,000 ticks |
| `horizon` | 100 | 1..1,000,000 ticks |
| `interval` | 1 | 1..1,000,000 ticks |
| `min_active` | 2 | 0..1,000,000 ticks |
| `switch_margin` | 1 | 0..1,000,000 priority units |
| `choices` | 4 | 1..32 |
| `context_observations` | 3 | 0..32 |
| `model_calls` | 1 | 0..32 complete provider calls per frame; explicit host reservations |
| `time_sources` | [] | unique subset of `sun,instrument,report,inference,routine` |

A frame has exactly `revision,now,conditions,allowed,emergency`, up to 32,768 bytes.
Conditions/allowed contain at most 128 unique scoped IDs; allowed activities must exist.
New revisions strictly increase; ticks never decrease. Non-emergency updates also require
`interval` elapsed ticks. Timelines are ordered by start then ID; each row has
`id,start,end,state,since,reason`. States are `planned,active,interrupted,completed,cancelled`.
Reasons are respectively `window,selected,preempted,host` and `preempted|missed`.
Expired terminal entries retire; this is a bounded rolling plan, not historical evidence.

An observation has exactly `token,observer,location,at,expires,kind,source,audience,value,
uncertainty,time`. At most 8,192 bytes each; arbitrary `value` at most 1,024 bytes.
Token is a positive monotonic u32, retained through eviction/clear. Observer is the bound
actor. Expiry must exceed observation tick by 1..retention. Audience equals the bound
participant set (at most 16 IDs). Kind is `direct|instrument|report|inference`.
Source uses existing `info` evidence fields `id,speaker,text`, with text up to 512 characters.
Uncertainty is null or nonempty host text up to 256 UTF-8 bytes without controls.

Time is null or exactly `source,calendar,low,high,rate_low,rate_high,usable`. Low<=high;
rates are integers 0..16 with low<=high, strictly less when usable. Calendar must match
the history. Source/kind pair must be `sun/direct`, `instrument/instrument`, `report/report`
or `inference/inference`. Usable sun requires `value.visible:true` supplied by a host sensor.
No history API accepts model proposals as observations.

Time knowledge field order is `status,sources,conflicting`. Status is `known|unknown`.
Each numeric source has `source,basis,observed_at,age,calendar,low,high,precision,uncertainty`.
Precision is `exact` only for a zero-width instrument interval, otherwise `range`.
Routine sources instead have `source,basis:"routine",phase,precision:"approximate"`.
Sources are newest first, capped by context_observations (routine phase may add one).
Conflicting means any same-calendar intervals are disjoint; no source is automatically
preferred. Expired/unusable/disallowed sources are absent; unknown has no sources.

`routine_status` arguments are `{}`; result order is `revision,current,next,observations,time`.
Current/next are null or `id,location,state`; simulation clock/windows stay host-side.
`activity_propose` arguments use the existing exact `Choices` action/target pair schema,
action `activity`. Result is `status:"proposed",revision,activity,start`; no mutation.
Both use existing character argument/result limits and per-turn exposure. Live status
permission is action `routine_status`, target bound actor. Proposal permission is the
exact activity pair. Host operations cannot be called through model dispatch.

Runtime saves have exact fields `version:1,scope,actor,participants,calendar,config,definition,
frame,timeline,history,completion_token,calls`, at most 1,500,000 bytes. History saves have
`version:1,scope,actor,participants,calendar,config,last,clock,records`, at most 1,200,000
bytes. Identity, config, definition and durable host-supplied watermarks must match;
restore validates every record/occurrence and commits atomically. Saves are trusted host
data, not authenticated tokens. Empty initial runtime snapshots can round-trip with zero
watermarks. Initial history-only runtimes use null definition/timeline. Ordinary clear
retains frame/replay watermarks; intentional host replacement requires durable safeguards.

`local` workflow kind `routine-fixture` takes exactly `definition,defaults,overrides,frames,
observations,now`; at most 128 frames/observations. Its result is `timeline,time`.
It evaluates timeline and observation history independently for offline byte parity,
not live ingestion order, provider rendering or model behavior. Twenty-one new fixtures
are additive, including six expected errors.

### Explicit observation selection and timer hints

`observation::Selection` takes exactly `kinds,locations,time_sources,required,limit`,
at most 8,192 bytes. Kinds are a unique subset of `direct,instrument,report,inference`;
locations at most 16 unique scoped IDs; time sources a unique subset of
`sun,instrument,report,inference`. Empty filter arrays do not restrict that dimension.
Required is at most 32 unique positive u32 native tokens. Limit is 0..32, at most the
configured context_observations, and at least required count. Every required token must
still be retained, unexpired, and match all filters. Fill remaining slots newest first;
return full records in original observation order. Never mutate or renew evidence.
`Runtime::context_for` uses these records for both observations and numeric time sources.

`character_routine_context` is a host-only read with `id,selection,revision,max_bytes`;
max_bytes is 64..32,768 and the live runtime revision must match. The result has the
unchanged context shape. `character_routine_wake` (`id`) returns `revision,at`; both can
be null before a frame, and at is null if no bounded future timer exists. Wake ticks
respect the runtime interval and u32 bound. `character_routine_affected`
(`id,conditions,activities`) returns `affected`; each list has at most 128 unique scoped
IDs. These operations do not expose tools or invalidate a counted frame by themselves.

Offline kind `routine-detail` takes the `routine-fixture` fields plus
`selection,changed_conditions,changed_activities`; result order is
`observations,time,next_wake,affected`. Nine additive fixtures cover selection, timers,
required-source errors and bounds. No previous expected outputs change.


### Typed time assessment and restored lifecycle checks

Opt-in `TimeAssessment::to_value()` has fields `kind,knowledge`. Kind is
`unknown|approximate|exact_reading|reported|conflicting`; knowledge retains the existing
`status,sources,conflicting` object. Conflict takes precedence, then unknown/empty,
then any exact instrument reading, then all-emitted-sources-report, else approximate.
These are host answer categories, not calibrated confidence or simulation truth.
`character_routine_assess_time` accepts exactly `id,revision,max_bytes` (64..32,768),
requires the live revision and returns this shape. It is not model-callable.

Existing time-summary conflict detection now considers all usable authorized records
in its supplied view, even beyond the source output cap. Numeric allowance zero reveals
no numeric conflict. Sources excluded by explicit task selection remain excluded.
Non-planned restored rows require `since>=start`; interrupted/preempted-cancelled rows
must agree with the authored resume policy. A restored frame under the current revision
must match it exactly, including conditions, permissions, tick and emergency status.

Offline `local` kind `time-assessment` accepts exactly
`scope,actor,participants,calendar,defaults,overrides,observations,now,phase`; phase is null
or two host-scoped IDs `[routine,activity]`. It is an independent host-history fixture,
not an engine sensor or model operation. Ten cases include four errors; five further
conflict cases exercise caps. All 827 earlier cases retain their bytes.

The example adapter's optional count receipts record `key,reused,input_tokens,
source_preflight,source_generation,status`. Source indices cite complete calls agreeing
on count. Keys bind the entire request, output, context and model/template/runner.
These are host telemetry, not a new packed/core format. Invalid neutral transport
controls, nonpositive output or insufficient whole-turn allowance fail before inference.

## Optional dynamic characters

Existing formats remain unchanged. Definitions, source/subjective views, standalone
checkpoints and context packets use new version 1 contracts; the persistent combined
checkpoint is version 2. [LIFE](LIFE.md) describes host authority and recovery.

- `Setting`: exactly `id,calendar,entities,history`. Up to 128 entities/history records;
  each history record is `id,at,participants,text`. Drafts reference only approved history.
- `Definition`: exactly `version,id,revision,setting,calendar,facets,attitudes,rules,history,
  improvisation`; at most 65,536 bytes. See editable `examples/characters/merchant-life.json`
  for every nested field. Maximum 64 facets/attitudes/rules, 32 history IDs; texts are
  1..512 non-control characters and at most 2,048 UTF-8 bytes. IDs use existing scope bounds.
- Life config: optional `enabled,balanced,deception,mistaken,tools`, all features false and
  tools empty by default. Resolve shared defaults then partial overrides. Requested tool
  names are only `life_recall` and `life_change`; they still require exposure/live permission.
- An experience is exactly `token,at,kind,tag,subject,source,audience,text,causes,retain`.
  Kind is direct/report/intended/outcome/correction; source uses original `id,speaker,text`.
  Tokens strictly increase, ticks never go backward, audience equals the fixed scope.
  Up to eight earlier causal tokens. Working history/change log each hold at most 128 rows.
- `life_recall({tags,limit})`: up to eight tags and limit 0..16. `life_change({rule,event,
  reason})`: bounded proposal only. Host applies an authored rule against actual direct/
  outcome evidence with current life revision; delta -20..20, nonzero, levels -100..100,
  cooldown 1..1,000,000 ticks and reversal through zero. Result cap 64..8,192 bytes.
- Scene: `id,revision,epoch,at,participants,facts,affordances`; up to 32 facts/16 affordances.
  Facts: `id,revision,observed_at,source,kind,audience,text`. Kinds global/perceived/mutual/
  report remain distinct; global facts never enter character context. Epoch is monotonic and changes when
  permissions/audience/emergency invalidates pending work. Selected source records match
  exactly; unrelated global revisions need not cancel dialogue.
- Thread metadata: `id,topic,questions,promises,unfinished,listener,participants,shared,
  referents`; at most eight threads/open questions, 16 IDs per metadata list. Existing
  `dialogue::Thread` supplies up to 128 whole exchanges for bounded tail selection.
- `ContextCaps`: `facts,facets,attitudes,episodes,exchanges`, maxima 32/16/16/16/16.
  Focused defaults 4/2/2/2/1; explicit expanded defaults 16/8/8/8/4. Selection is exactly
  `required,optional,tags,input`: required facts must fit; only named authorized facts
  qualify. Optional causal recall groups remain whole. Packet and host inspection are separate.
- Delivery: `id,event,through,recipient`; through is a cumulative Unicode character
  offset. Generated/validated/queued/played/heard/interrupted/cancelled are distinct.
  Heard cannot exceed played. Maximum 32 live lines/eight pending tickets, TTL 20 ticks.
- Life checkpoint: `version,identity,scope,actor,participants,definition,config,state,
  archive_ack,summaries`; identity `campaign,branch,instance`, maximum 524,288 bytes.
  Restore fence: `min_revision,max_revision,min_token,max_token,max_tick`. Up to 16
  complete nonoverlapping extractive ranges, 128 originals per range, idempotent by range.
- Initiative candidates: `id,kind,priority,reason,requires,resource,recipient,attitudes,
  events,promises`. Up to 16 candidates, eight attitude/event references, four accepted
  promise IDs; kind activity/ask/followup/decline/gesture/silence. Frames:
  `revision,now,turn,conditions,allowed,resources,audience,emergency,depth,social_remaining`.
  See LIFE for bounded policy defaults, one-use dispatch and confirmed outcome receipts.

`Value::is_valid(cap)` checks exact compact size, parser-equivalent depth, JSON number
syntax and key uniqueness directly, with no heap allocation even on rejection. It uses
small-object comparisons and fixed-stack sorted blocks for wide objects. `request::bounded`
uses it; error construction still creates the existing owned error message. Serialization
bytes and all earlier expected files remain unchanged.

## Turn-aware life speech v2

Legacy context/request outputs stay unchanged. `Life::compile_speech` takes exactly
`version:2,rules,tags,caps,stable_scene,state,dialogue,input,tools,turn`. Rules and input keep
the existing 512-character/2,048-byte single-line bound. Tools hold at most 32 schemas;
selected whole dialogue must fit the declared exchange cap. The complete rendered result
is bounded and must be recounted for the actual provider.

`turn` is exactly `profile,knowledge,creative,speaker,question_subject,facts`. Profile is
focused/expanded, knowledge known/partial/unknown, creative is a host boolean. Each of at
most 32 facts is exactly `key,subject,status,value`, using approved entity IDs. Known values
are non-null; unknown values are null with status unknown. Each value is at most 1,024 JSON
bytes. Keys are unique scope strings, excluding question_subject/utterance_speaker/
fact_subjects. Facts must fit the attention cap, never drop required unknown markers.
Profile/known-state gating is described in [LIFE](LIFE.md#turn-aware-speech-policy-v2).

The additive `local` kind `life-speech-v2` is an offline adapter with `life,request`; it
returns `character,request`. It makes no model calls. `sentence-boundary` takes `text,
max_chars` and returns `characters,boundary_complete,single_sentence,at_schema_wall,
over_limit,suspected_cutoff`. Bounds are 1..1,024 max_chars and 1,024 Unicode characters /
4,096 bytes of text. Empty text is diagnosable. This punctuation heuristic preserves raw
text and does not certify semantic completeness or factual support.

## Archetype and party layer (opt-in v1)

[Complete contracts and host responsibilities](ARCHETYPES.md). Blueprint input is exactly
`version:1,preset,size,modules,policies,tools,model_calls`; presets are ambient/service/quest/
companion, sizes 0b/1b/1.5b/3b/8b/30b/70b. Module overrides are partial booleans; policies
are exactly balanced/deception/mistaken. Tools are a unique subset of life_recall/life_change
under module/tier requirements, with at most eight reserved turn calls. Resolved output
includes module flags, attention caps and existing life/routine/initiative configurations.

A scene is exactly `version:1,id,epoch,revision,participants`; participant records are
`id,kind` (player/ai), with 1..16 unique ASCII names. Receipts are exactly
`token,speaker,recipients,heard,text,parent`, bounded to 128 records, monotonic u32 tokens,
512 text characters / 2,048 bytes, and an earlier existing parent or null. No receipt is
authoritative evidence of a quoted factual claim. Up to 16 messages can be rendered.

Gate rules are `kind,id,clauses` (lore/facet/affordance); up to 128 unique kind/ID rules,
1..8 clauses each, exactly `attitude,target,op,value`, target speaker/listener, op ge/le,
value -100..100. Gate/item IDs retain existing bounded scope strings. Live gate frames are exactly
`revision,speaker,listeners` and are validated against current Life. Multi-listener clauses
apply to every listener; missing/ambiguous attitudes fail closed. Gate diagnostics are
host-only. Existing audiences, knowledge grants and subject scopes still apply.

`PartyScene::floor` takes `turn,now,trigger,policy,spotlight,candidates`. Floor policy is
`enabled,social_limit,model_calls,max_depth` with maxima 16/8/4 and disabled by default.
Spotlight is `enabled,significance` (threshold 1..100), disabled by default. Each of at most
16 unique AI candidates is `actor,priority,focused,expanded,signal`; priority -100..100,
expanded may be null. Routes are `name,size,calls,cost_micros`, calls 0..8, microdollars
0..1e12; zero-call routes cost zero. Signals are `significance,changes,conflicts` with
significance 0..100, conflicts 0..16 and at most 16 `before,after,threshold` crossings
bounded to -100..100. Results include selected one-use grants, omission reasons and shared
turn counters. Scene instances retain at most 16 reserved / 128 total grants.

`SceneTurn` borrows Life, Blueprint, gates, Lore and the host request. Request fields are
exactly `trigger,revision,life_revision,listeners,subjects,tags,turn,rules,stable_scene,
lore_select,affordances,tools`. Turn follows the existing SpeechTurn contract; lore_select
follows Lore::select and must identify the active AI. At most 32 affordances have exactly
`id,text,audience`. Up to two actually configured/exposed Life schemas are permitted.
The result is `request,refusal,inspection`; a denied required lore card yields a null
request and deterministic deferral. Full request bytes and all ordinary Life caps apply.
Stable scene/rules and every non-gated host field must already be redacted.

Additive `local` kinds: `archetype` resolves a blueprint; `party` replays exactly
`scene,ledger,operations` (ledger limit/window, up to 256 operations receipt/render/floor/
dispatch/cancel/revision); `party-compile` takes `life,blueprint,gates,scene,receipts,lore,
request`; `attitude-gates` is an offline fixture with `rules,actor,speaker,listeners,
participants,revision,expected_revision,attitudes,kind,items`. Its compact attitude rows
are kind/subject/level. These adapters are host/testing operations, never model tools.

## Living world / cast pack v1

[Full contracts, bounds and host responsibilities](LIVING_WORLD.md). CastPack is exactly
`version:1,id,revision,setting,players,locations,groups,characters,lore,features,propagation,
ripple,habits`, at most 1 MiB. Setting, Locations, Lore, Blueprint, Definition, Routine and
AttitudeGates reuse existing formats. Up to 12 characters plus up to 8 players must fit
16 total participants. Group records are `id,members`; places/groups must be approved
setting entities, with unique role IDs. Each character is `id,blueprint,definition,routine,
gates,affordances,home,fallbacks`. Routine presence matches the blueprint. Fallbacks contain
exactly calm/busy/tense/emergency/unknown single-line terminal-punctuation strings, at most
240 characters. All structured cross-cast references are checked before acceptance.

Features are exactly rumors/ripples/habits/atmosphere booleans. Propagation is exactly
`significance,trust,max_hops,per_tick,checks`, bounded to 0..100, -100..100, 0..4, 0..16,
0..1024. Ripple is exactly `numerator,denominator,max_delta,kind,groups,reason`; denominator
1..100, numerator 0..denominator, max_delta 0..5. Habit rules are exactly
`id,counter,threshold,text,tags` for fulfilled/broken/interrupted counters and thresholds
1..1,000,000. At most 16 group/habit records are permitted.

World observe is exactly `id,actor,event,significance,share_with,appraise`; it uses an
existing actual Life event (direct/outcome only), an optional authored rule ID and explicit
NPC sharing allowlist. World tick is exactly `now,windows,routines`. At most 16 windows
are `id,location,start,end,participants,kind`, kind scene/routine. Up to 12 routine updates
are `actor,frame` using existing Frame. Tick output is `routines,reports,blocked,
checked_pairs,revision,model_calls`. Scanning uses a rotating cursor, not unbounded closure
propagation. Reports include root/sender/actor/hops/path/original_source/event; recipient
Life events always have kind report. Group effects and effective levels remain separate
from unchanged base Life checkpoint formats. Capacities: 64 roots, at most roots×NPC
known holders, 128 influence records, existing per-actor Life limits.

Counter receipts are exactly `token,player,at,fulfilled,broken,interrupted,source`; counts
are monotonic cumulative integers 0..1,000,000, with existing source structure. Exact last
receipt replay is idempotent. Atmosphere updates are `location,revision,tag,source`, tag
calm/busy/tense/emergency, with strictly increasing per-location revisions. Selection is
`actor,players,tags,location,limit`, limit 0..4. Context returns badges/atmosphere/rumors/
group_influences; focused compiler selection further caps each extra list at two.

SpeechQueue uses existing RequestContext fields with an empty facts list; other dependencies
require current host revisions. Opaque capabilities are instance-bound, never serialized.
Issue accepts context/now/ttl/fallback; ttl 1..1,000,000, at most 32 retained entries. Complete
accepts at most 4,096 bytes of strict JSON with exactly speech (1..240 characters), checks
freshness/terminal punctuation and requires host semantic/permission validation. Invalid or
stale completions consume their attempt, retain raw text and fallback. Freeze seals the
chosen line before delivery; cancel/retire control lifecycle and storage. No hearing is
implied. IDs/clocks remain u32-bounded; model/tool/proposal authority does not change.

New host-only `local` kinds: cast-pack returns a validated summary; living-world replays
`pack,scope,operations` (at most 256 observe/tick/counters/atmosphere/context/inspect/initiative
operations); living-compile takes `world,scene,receipts,actor,request,selection`; speech-tickets
replays at most 256 issue/complete/freeze/cancel/retire operations. The speech fixture's
approved boolean stands in for a host validator, not model approval. These adapters are
never model tools and use the unchanged version-1 bridge/native ABI.

## Optional visual specs and asset registry

The additive `visual-spec` / `visual-assets` local formats and persistent `visual_*`
host operations are specified in [VISUAL](VISUAL.md). They add 64 conformance cases,
including 13 malformed inputs; all 1,026 existing cases retain their bytes. Bible/spec
objects use canonical lexical keys for SHA-256 identity (other formats retain their
existing ordering). Inspection is outside the key. Registry outputs have stable
`key,state,asset,exact,pending,generation,reason` order; submitted candidates remain
unapproved. Repair outputs `key,state,invalidated`. No model-call schema is added.
