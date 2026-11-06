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
| `table` | array of objects | `group_by` (column name or list); `formulas` (definitions for `derive`); view keys `derive`, `where`, `relevant_to`, `relevant_fallback`, `sort_by`, `limit`, `fields`, `round` as in the manifest | packed table (view applied first) |
| `signatures` | array of tools | | the signatures text, as a JSON string; unpacking must give back the tools without any `{"type":"function"}` wrapper |
| `schema` | array of tools | | packed schema |
| `outline` | array of tools | | the outline, as a JSON string |
| `catalog` | catalog (object or bare array) | `query` or `relevant_to`, `kind`, `offset`, `count`, `expected_generation`, `limits` (any of `entries`, `tags`, `results`, `query_chars`, `query_words`, `offset`) | one search page |
| `budget` | array of `{name, priority?, stable?, required?, value}` | `max_bytes` (no limit when absent) | `{"packet": ..., "receipt": ...}` |
| `estimate` | `{scope: [strings], observations: [[body_bytes, input_tokens, estimate_used], ...], body, framing}` | | `{"calibration": ..., "estimate": n}`. Observations are applied in order, starting uncalibrated; the estimate uses the same scope. |
| `formula` | `{define: [formulas], calc: [expressions]}` | | each expression's result, or `null` where evaluation fails |
| `query` | object of table names to arrays of row objects | `sql` (a SELECT string) | selected values or aggregates; see [Queries](../docs/FORMAT.md#queries) |
| `query-schema` | object of named tables | | column types and complete small string domains |
| `query-session` | object of named tables | `calls` (argument objects), `result_bytes` (default 4096) | one feedback envelope per call; requests after a terminal reply are errors |
| `brain`, `brain-select` | exactly one of `definition`, `dense`, `state`, plus `operations:[{op,args}]` | | `{"results":[...],"state":...}`; see [Reaction brains](../docs/FORMAT.md#reaction-brains); `brain-select` cases exercise opt-in rate selection |
| `brain-cache` | same workflow as `brain` | `capacity` (default 32, range 0–256) | identical results/state through an ephemeral FIFO prediction cache; includes mutation, disabled-cache and error cases |
| `memory` | saved memory | `relevant_to`, `count` (default 8) | the recalled records as a table |
| `fingerprint` | array of strings | | array of 16-hex-digit FNV-1a fingerprints |

## Changing behavior

`runtime` cases select a native API workflow with `input.kind`: `infer` (millionth
input arrays/mask/thresholds), `budget` (sections/cap), `schedule` (registrations/events),
`job` (one training workflow plus slice cap), `host` (host-policy selection workflow),
or `sync` (publish/deliver/apply operations). The reference adapter is
[`tests/support/runtime.rs`](../tests/support/runtime.rs); Python expectations come from
[`eval/runtime_eval.py`](../eval/runtime_eval.py). These adapters exercise native APIs;
they are not additional model tools. Error cases cover input/work bounds and invalid
stream, revision, order, removals and scheduler events.

`integration` cases use `input.kind` to exercise `bindings`, `metadata`, `diagnostics`,
`stability`, `replay`, `journal` or `evidence`. The native adapter is
[`tests/support/integration.rs`](../tests/support/integration.rs), with Python references
[`integration_eval.py`](../eval/integration_eval.py) and [`evidence_eval.py`](../eval/evidence_eval.py).
Error cases include incompatible saved meanings, malformed policies/citations, stale
snapshots, privacy changes, event-token replay and missing dependencies. These are
native API adapters, not additional default model tools. See the corresponding
[format sections](../docs/FORMAT.md#versioned-feature-bindings).

The Rust reference runner is `tests/conformance.rs`.

`facts` workflows contain `capacity` and `operations`: register, check, packet, unpin,
forget and forget_privacy. The native adapter is
[`tests/support/facts.rs`](../tests/support/facts.rs), with Python expectations in
[`facts_eval.py`](../eval/facts_eval.py). These exercise native registration and read-only
claim checks, not a model registration capability. Cases cover typed numeric agreement,
stale/cyclic/expired dependencies, pins, user precedence, scoped lookup, privacy erasure,
byte budgets and clock bounds. See [registered facts](../docs/FORMAT.md#registered-facts).

After a deliberate behavior change:

```bash
GAMECTX_BLESS=1 cargo test --test conformance
git diff conformance/
```

Review every changed `expected.json` before committing. Blessing records what the code
does now, not what it should do. A changed `expected.json` is a **format change** and
must be listed as one in the CHANGELOG.
