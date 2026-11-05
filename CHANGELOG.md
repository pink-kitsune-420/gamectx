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
