# Conformance cases

These cases define gamectx's behavior independently of the Rust code. A port in another
language, or a project that reimplements part of gamectx, runs the same cases with its
own small runner and is conformant when they all pass.

Each case is a folder `<op>/<name>/` containing:

| File | Meaning |
|---|---|
| `case.json` | `{"op": "...", "args": {...}, "error": true?}` |
| `input.json` | The input value. For `parse` cases it is `input.txt`, raw text. |
| `expected.json` | The exact output, in [compact serialization](../docs/FORMAT.md#compact-serialization), followed by one newline. Absent for error cases. |

A case passes when:

- **Success cases:** the output, serialized compactly, equals `expected.json` without its
  final newline, **byte for byte**. For `table`, `signatures` and `schema`, unpacking the output must
  also give back the input (ignoring object key order). Tables with a view give back the
  viewed rows; grouped tables give them back in group order.
- **Error cases** (`"error": true`): the operation is refused. The message is not part of
  the contract.

## Operations

| `op` | Input | `args` | Output |
|---|---|---|---|
| `parse` | raw JSON text | | the parsed value |
| `table` | array of objects | `group_by` (column name or list); view keys `where`, `relevant_to`, `relevant_fallback`, `sort_by`, `limit`, `fields`, `round` as in the manifest | packed table (view applied first) |
| `signatures` | array of tools | | the signatures text, as a JSON string; unpacking must give back the tools without any `{"type":"function"}` wrapper |
| `schema` | array of tools | | packed schema |
| `outline` | array of tools | | the outline, as a JSON string |
| `catalog` | catalog (object or bare array) | `query` or `relevant_to`, `kind`, `offset`, `count`, `expected_generation`, `limits` (any of `entries`, `tags`, `results`, `query_chars`, `query_words`, `offset`) | one search page |
| `budget` | array of `{name, priority?, stable?, required?, value}` | `max_bytes` (no limit when absent) | `{"packet": ..., "receipt": ...}` |
| `estimate` | `{scope: [strings], observations: [[body_bytes, input_tokens, estimate_used], ...], body, framing}` | | `{"calibration": ..., "estimate": n}`. Observations are applied in order, starting uncalibrated; the estimate uses the same scope. |
| `memory` | saved memory | `relevant_to`, `count` (default 8) | the recalled records as a table |
| `fingerprint` | array of strings | | array of 16-hex-digit FNV-1a fingerprints |

## Changing behavior

The Rust reference runner is `tests/conformance.rs`. After a deliberate behavior change:

```bash
GAMECTX_BLESS=1 cargo test --test conformance
git diff conformance/
```

Review every changed `expected.json` before committing. Blessing records what the code
does now, not what it should do. A changed `expected.json` is a **format change** and
must be listed as one in the CHANGELOG.
