# Conformance cases

These cases define gamectx's behavior independently of the Rust code. A port in another
language, or a project that reimplements part of gamectx, runs the same cases with its
own small runner and is conformant when they all pass. The v0.3.0a suite contains 747
cases; all 496 cases from v0.2.0a retain their original inputs and expected outputs.

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
| `kit` | `{project,op,args}` | | version-1 registration/build/query/tool-validation/feature/evaluation workflow; see [integration kit](../docs/FORMAT.md#integration-kit). Errors reject the workflow; bridge error text is diagnostic. |
| `local` | `{kind,args}` | | local deployment and optional optimization workflows; see [contracts](../docs/FORMAT.md#local-model-workflow-contracts) and [optimization additions](../docs/OPTIMIZATION.md). |
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
| `query-guidance` | JSON profile string (`standard` or `explicit-selection`) | | supplemental system text as a JSON string; unknown names/wrong types are errors |
| `query-plan` | `{sql, shape?, op?: "plan" or "run", rows?, max_rows?, result_bytes?}` | | query structure; optional strict host shape; bounded execution returns `{plan,result}`. See [query plans](../docs/FORMAT.md#query-plans-and-host-contracts). |
| `query-schema` | object of named tables | | column types and complete small string domains |
| `query-session` | object of named tables | `calls` (argument objects), `result_bytes` (default 4096) | one feedback envelope per call; requests after a terminal reply are errors |
| `brain`, `brain-select` | exactly one of `definition`, `dense`, `state`, plus `operations:[{op,args}]` | | `{"results":[...],"state":...}`; see [Reaction brains](../docs/FORMAT.md#reaction-brains); `brain-select` cases exercise opt-in rate selection |
| `brain-cache` | same workflow as `brain` | `capacity` (default 32, range 0–256) | identical results/state through an ephemeral FIFO prediction cache; includes mutation, disabled-cache and error cases |
| `memory` | saved memory | `relevant_to`, `count` (default 8) | the recalled records as a table |
| `fingerprint` | array of strings | | array of 16-hex-digit FNV-1a fingerprints |

## Changing behavior

The `lore/` family adds 28 local cases for bounded world loading/selection and source-bound
answers. `eval/lore_cases.py --cap 0 --check` compares 108 independent workflows;
all 633 prior cases stay unchanged. [Exact contracts](../docs/LORE.md).

The `conversation/` family adds 38 `op:local` fixtures for shared conversation retention,
reply validation, entity references and explicit confirmation. Independent Python
prototypes compare 78 workflows with `eval/conversation_cases.py --cap 0 --check`;
all 595 prior cases remain unchanged. [Contract](../docs/CONVERSATION.md).

The `workflow/` family adds 30 `op:local` cases: 24 size/task profiles, two direct
renders and four errors. Independent expectations come from `eval/workflow_reference.py`
and `eval/workflow_cases.py --cap 0 --check`. Existing `preset`, `render_chat` and all
565 earlier cases remain unchanged. The same cases pass through the persistent and
native bridges with `--check-bindings`. See [WORKFLOWS](../docs/WORKFLOWS.md).

The `optimization/` family adds 69 cases using `op:local`. Independent outputs come
from `eval/optimization_reference.py`; `optimization_cases.py --check` compares 179
accepted/rejected workflows against Rust. They cover host intent, prepared predicate
binding, evidence dependencies, external counts, local routing, readable decisions
and split configuration comparison. All 496 v0.2.0a cases remain unchanged.

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

`upgrades` workflows select `kind`: recall, playbook, outcomes, temporal, bundle, requests
or ledger. The native adapter is [`tests/support/upgrades.rs`](../tests/support/upgrades.rs)
and independent expectations come from [`eval/upgrades_eval.py`](../eval/upgrades_eval.py).
The 40 cases include 18 positive/rejection-report workflows and 22 errors. Current versus
issued request contexts are independent snapshots; changed actor/world/schema/permissions
must produce false. Other cases cover stale fact versions, one-use labels/tickets, absent
exposure, invalid phases, original event clocks, aliases, imported ports and reservations.
These exercise native APIs, not new default model tools.

After a deliberate behavior change:

```bash
GAMECTX_BLESS=1 cargo test --test conformance
git diff conformance/
```

Review every changed `expected.json` before committing. Blessing records what the code
does now, not what it should do. A changed `expected.json` is a **format change** and
must be listed as one in the CHANGELOG.
