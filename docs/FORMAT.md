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
