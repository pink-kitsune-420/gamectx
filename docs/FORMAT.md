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
| `budget_tokens` | integer, optional | Maximum estimated input tokens (see [Calibration](#calibration)). If both budgets are set, the packet must fit both. |
| `sections` | array | The sections, in declared order. |

Each section:

| Field | Type | Default | Meaning |
|---|---|---|---|
| `name` | string | required | Key in the packet; must be unique. |
| `pack` | string | `raw` | `raw`, `table`, `signatures`, `schema`, `outline` or `catalog`. |
| `value` / `file` | any / path | one required | Inline data, or a JSON file relative to the manifest. |
| `priority` | integer | 0 | Higher is more important. Lower priorities are dropped first; ties drop the later-declared section first. |
| `stable` | bool | false | Rarely changes between requests. Stable sections are placed first, keeping the start of the packet identical so it can be cached. |
| `required` | bool | false | Never dropped. Assembly fails if required sections alone exceed the budget. |
| `group_by` | string | none | `table` only: group rows under this column's values. The column must hold strings. |
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

Unpacking a grouped table restores every row and value, with rows in group order and the
group column first in each object. Values are never replaced with lookup indexes; see
the measurements in the README for why.

### Views

A table section can shape its rows before packing. Steps always run in this order:

1. `sort_by`: stable sort by one column. Numbers compare as numbers, strings by Unicode
   code point; a column that mixes the two is an error. Nulls go last in both directions.
2. `limit`: keep the first N rows.
3. `fields`: keep only the listed columns, in the listed order. Sorting first means you
   can sort by a column you do not send, such as distance to the player.
4. `round`: round numbers in a column to N decimals (0–12), trimming trailing zeros
   (`3.50` → `3.5`, `-0.04` at 1 place → `0`). Numbers without a fraction or exponent are
   left exactly as written. Nulls are left alone; other values are an error.

A view drops information on purpose, so `verify` reports such sections as "round-trips
the rows its view selected": the packed table restores exactly the viewed rows.

### `signatures` (lossless)

The recommended way to describe actions in a prompt: tool schemas as short text lines,
packed into the packet as one JSON string. Input: the same tool array as `schema`.

```text
All arguments are required; no other arguments are allowed.
Shared arguments (same meaning wherever listed):
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
- `generation` is a 64-bit FNV-1a fingerprint of the catalog. Passing it back
  (`--generation`) makes a later page fail instead of mixing old and new data. It
  detects changes; it is not a security hash.
- A trailing `"..."` tag means the entry had more tags than were kept.

## Receipt

`gamectx pack --receipt` writes this to stderr:

```json
{"columns":["name","bytes","priority","stable"],
 "included":[["actions",1045,10,true],["rules",142,9,true],["inventory",444,6,false],["recent_events",227,7,false],["mood",37,8,false]],
 "dropped":[["action_outline",314,1,true],["lights_nearby",376,3,false]],
 "total_bytes":1954}
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
