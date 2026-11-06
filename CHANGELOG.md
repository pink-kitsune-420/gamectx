# Changelog

While the version is `0.x` or a pre-release, any release may change the API,
command-line options or packed formats. Every entry carries one label so projects
know what an upgrade costs:

| Label | Meaning | What projects using gamectx should do |
|---|---|---|
| **[format]** | Packed output or accepted input changes; one or more `conformance/` expected files changed. | Update ports, rerun conformance, and recheck model decisions that depend on the changed shape. |
| **[api]** | Rust library or command-line interface changes; output is the same. | Update calling code. |
| **[internal]** | Faster, safer or clearer with identical behavior; conformance files untouched. | Upgrade freely. |
| **[docs]** | Documentation only. | Nothing. |

## Unreleased

- **[api]** Adds allocation-free `Brain::run_into`, reusable `Prediction` and fixed-point
  `SCALE`; existing JSON inference uses the same kernel and keeps its output.
- **[api]** Adds disposable `TrainingJob`s for single-rate, selection and host-policy
  selection. Sample slices preserve training order/results; finishing rejects stale
  state and dropping cancels without mutation. Adds `schedule::Scheduler` for bounded
  event/urgency/refresh scheduling.
- **[api]** Adds `PreparedSection` and reusable packet assembly; `Context` retains encoded
  sections. Exact byte-budget fitting avoids repeated packet clones/serialization.
  `json_len()` counts exact serialized bytes without allocating. Generic fitting
  predicates preserve their complete-candidate behavior.
- **[format]** Adds `train_select_host` workflows with per-validation eligibility and
  thresholds, and acknowledged `sync` envelopes. Adds 35 `runtime` conformance cases;
  all previous 226 cases remain unchanged. Default model tool schemas are unchanged;
  hosts opt into the new validation operation.
- **[api]** Adds bounded `sync::Publisher` / `Replica` with explicit stream/revision
  checks, snapshots on missing ACK, exact reconstruction and atomic rejection.
- **[docs]** Free offline measurements: native inference 5.3–13.2× faster than JSON in
  two changing-state workloads, zero allocations; budget-pressure assembly about 7.7×
  faster one-shot under pressure; already-fitting one-shot builds were about 9% slower
  with fewer allocations. Synthetic scheduling 15,360 → 512 evaluations; synchronization
  489,490 → 19,089 transfer bytes. Host-policy selection ties existing selection at
  1,466/1,536 held-out decisions. Local-server prefix-cache harness added; actual KV
  cache savings remain unmeasured. Timings are local workload results, not guarantees.

- **[api]** Adds `brain::CachedBrain`, `CacheStats`, `process_cached` and CLI
  `brain WORKFLOW --cache N`. Bounded FIFO prediction caching is opt-in, clears on
  mutable access and never enters saved learning. Preload once and clone compiled
  templates for independent owners. Local release measurements: 1.7–2.0× faster for
  repeated states, 1.8–1.85× slower with no hits; template cloning 15.9–17.2× faster
  than restoring saved JSON for each actor. No API calls.
- **[format]** Adds six `brain-cache` conformance cases exercising the existing workflow
  through the optional cache, including errors. All 220 previous cases are unchanged;
  cached results and saved learning use the existing format.
- **[format]** Adds opt-in `train_select` brain workflows/reports and the
  `train_select_brain` tool; 17 new conformance cases. All 203 previous cases stay
  byte-identical, including existing brain training, inference and saved learning.
- **[api]** `Brain::train_select` compares bounded rates using validation reaction counts
  and MSE, with an explicit aggregate cap, one retained snapshot, and rejection of MSE
  improvements that worsen validation choices. Paired fresh synthetic tests: 1,436/1,536
  single-rate → 1,487/1,536 selected-rate; nonlinear task 315/384 → 359/384, at about 3×
  estimated training work. Inference cost/format unchanged; no API spend.
- **[format]** New `brain` graph, saved-learning and workflow formats, with 41 conformance
  cases. All 162 previous cases remain byte-identical. Neural arithmetic and training use
  integer millionths, including canonical decimal serialization.
- **[api]** `brain::Brain`: game-defined acyclic reaction networks, named normalized inputs,
  eligible-output masks and abstention; bounded supervised training and weight lessons on
  candidate copies, validation-loss gates, pins, learning limits, revisions/replay protection,
  persistence and eight retained undo snapshots. Model tools and `gamectx brain` workflow CLI.
- **[docs]** Offline tests across four synthetic policy tasks: random initialization
  545/1,536 → trained 1,421/1,536 on fresh held-out samples; changed trading preference
  264/384 → 384/384. Includes weaker nonlinear results and rejected training variants.
  No LLM accuracy or billed-token saving is inferred from these local tests; API spend $0.
- **[format]** Adds `query-schema` descriptions (types and complete small string domains)
  and `query-session` result/error envelopes, with 23 conformance cases. All 139 previous
  cases remain byte-identical.
- **[api]** `query::describe` and `query::Session`: validate tool arguments, cap result
  bytes and allow at most one correction after an execution error; success ends the
  session too. CLI: `query --schema` and streaming JSON-lines `query --session`.
- **[docs]** Explicit `unit_weight`/derived `stack_weight` host example and an eval that
  includes correction and final-answer turns, with separate development and holdout states.
  On 3 holdout states × 29 questions × 2 small models: full tables 171/174, schema-only
  172/174 after correction; average complete-exchange input 4,898.632 → 1,615.874
  (−67.0%), completed-exchange cost −64.1%. Development state is reported separately.
- **[format]** New `query` conformance operation: bounded, read-only model-written SELECT
  over named game tables, with filters, sorting, limits, arithmetic and aggregates.
  ORDER BY and LIMIT apply before aggregates. Adds 36 cases; all 103 previous cases stay
  byte-identical. No existing packed format changes.
- **[api]** `query::{tool, run, Query, Limits}` and `gamectx query TABLES SQL` /
  `gamectx query --tool`. Measured on the two small models: direct answers 33/58,
  SQL 53/58; numeric filters 0/6 → 6/6; arithmetic 1/16 → 12/16. A separate run with
  the tool packed as a signature scored 52/58. The full data was sent in both runs.
- **[internal]** Query replay and differential checks against the Python prototype;
  mutation fuzzing, limit checks and non-finite arithmetic coverage. Token-only evals
  enforce the spending cap and save usage and costs.

## 0.1.0-alpha.2 — 2026-11-05

Adds features without changing any alpha.1 output: all 51 alpha.1 conformance cases are
unchanged, and 52 new cases cover the additions.

- **[format]** New `signatures` pack mode: action schemas as short text lines with shared
  arguments listed once. Lossless for strict tools with string, number, integer, boolean
  and string-enum arguments; other tools are refused with a pointer to `schema`. Existing
  formats are unchanged. Measured: 12–14% fewer input tokens than `schema`, and 46% fewer
  than compact JSON overall, at equal or better accuracy.
- **[docs]** README and guide report a 53-question run including signatures.
- **[format]** `group_by` accepts a list of columns to nest groups (`["kind", "rarity"]`).
  A single column still produces the same output as before. Measured: nested grouping took
  two-column filter questions from 7/12 to 12/12 on the smaller models.
- **[api]** `Context` and `Turn`: register tools, prompts and constants once, add variables
  and tables per turn, and build a budgeted packet; `Packet::system_prompt` adds an intro.
  `Table` options cover grouping, views, priority and required.
- **[format]** Views gain `where` (exact filters: `=`, `!=`, `>`, `>=`, `<`, `<=`, `in`) and
  `relevant_to` (keep the rows that best match some text, or all rows if none match).
  Measured: `relevant_to` with each question's text used 26% fewer input tokens at
  identical accuracy. In Rust: `Table::filter` and `Table::relevant_to`.
- **[format]** `relevant_fallback: "none"` (Rust: `Table::relevant_only`) sends no rows when
  nothing matches. Measured: 59% fewer input tokens than `relevant_to` alone; only
  whole-table questions are lost.
- **[format]** Catalog sections take `relevant_to`: entries ranked by matching words of a
  message. Rust: `Catalog::relevant`, `Turn::catalog_relevant`, `Turn::catalog_search`.
- **[format]** New `memory` module and `memory` pack mode: records with turn, importance,
  text and tags; recall ranks by rare-word matches, then importance, then recency;
  `Memory::tool` and `Memory::apply_call` let the model write memories with validation;
  `Turn::memory` adds recalled records. Measured: 84% fewer input tokens than sending a
  60-event history, same accuracy.
- **[format]** New `formula` module: safe formulas (numbers and booleans, operators, `if`,
  `min`/`max`/`clamp`/`round`/..., calls to earlier formulas only) with definition-time
  checks and step limits; `derive` view columns; manifest `formulas`; `Formulas::tools`
  gives the model `define_formula` and `calc`; `gamectx calc` on the command line.
  Measured: models answered 0/16 arithmetic questions directly, 8/16 by writing an
  expression for gamectx to compute; the rest picked wrong rows.
- **[api]** `Value` converts from strings, numbers, booleans, `Option` and `Vec`, and the
  `obj!` macro builds objects.
- **[api]** `gamectx pack -` reads the manifest from standard input, for calling from any
  language.
- **[docs]** The Ember Keep example describes its actions with `signatures`; its budget is
  2000 bytes so it still shows two sections being dropped.
- **[internal]** `gamectx report` and `verify` columns fit the longer pack name.

## 0.1.0-alpha.1 — 2026-11-05

First public release.

- **[format]** `json`: strict, order-preserving parsing that keeps exact number text; compact serialization rules.
- **[format]** `table`: lossless header-plus-rows packing, optionally grouped by one column (`group_by`).
- **[format]** `view`: `sort_by`, `limit`, `fields` and `round` shape table rows before packing;
  `verify` reports viewed sections separately from exact round-trips.
- **[format]** `schema`: lossless tool-schema packing with shared `$defs`; lossy `outline` signatures.
- **[format]** `catalog`: searchable, paged registries with change-detecting generations.
- **[format]** `budget`: stable-first assembly that drops low-priority sections, with receipts.
- **[format]** `estimate`: conservative token estimates calibrated from provider usage reports.
- **[api]** `gamectx` command: `pack`, `report`, `verify`, `search`, `outline`, `calibrate`.
- **[docs]** 51 shared conformance cases defining every format, for ports in other languages.
- **[docs]** Model evaluation script (`eval/`), measured results in the README, and
  `docs/GUIDE.md` with practical rules drawn from them.
